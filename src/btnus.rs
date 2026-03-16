// use std::thread;

// use std::time::Duration;

use std::collections::HashMap;
use std::error::Error;

use bluest::{Adapter, AdvertisingDevice, Device, DeviceId};

use futures_lite::StreamExt;

// use flume::async::RecvStream;
use tokio::runtime::Runtime;
use tokio::time::{Duration, timeout};
use tracing::{debug, error, info, trace, warn};
// use tracing::{error, info, warn};

use uuid::Uuid;

/// UUID for NUS BLE Service
pub const NUS_SVC_UUID: Uuid = Uuid::from_u128(0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E);
/// UUID for NUS Rx (BLE writes to device) BLE Characteristic
pub const NUS_RX_CHR_UUID: Uuid = Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);
/// UUID for NUS Tx (BLE notifs from device) BLE Characteristic
pub const NUS_TX_CHR_UUID: Uuid = Uuid::from_u128(0x6E400003_B5A3_F393_E0A9_E50E24DCCA9E);

#[derive(Debug, Clone, PartialEq)]
pub enum ThreadedNusMsg {
    /// Command to start scanning
    DoScanStart(String), // FIXME: put scan params as a type in this event

    /// Command to stop scanning
    DoScanStop,

    /// Command to connect
    DoConnect(DeviceId),

    /// Command to disconnect
    DoDisconnect,

    /// Command to quit
    DoQuit,

    /// Scan result data
    DataScanResult(Vec<AdvertisingDevice>),

    /// NUS TX bytes (BLE notif from device)
    DataTx(Vec<u8>),

    /// NUS RX bytes (BLE write to device)
    DataRx(Vec<u8>),

    /// Not Ready State
    AmNotReady,

    /// Ready Idle State
    AmReadyIdle(String),

    /// Scanning State
    AmScanning,

    /// Connecting State
    AmConnecting,

    /// Connected State
    AmConnected,

    /// 'Quitted' State
    AmQuitted,
}
use ThreadedNusMsg::*;

/// async function to handle connection and active use for NUS data transfer
async fn bt_nus_setup_and_loop(
    adapter: &bluest::Adapter,
    bt_id: &DeviceId,
    cmd: &flume::Receiver<ThreadedNusMsg>,
    resp: &egui_inbox::UiInboxSender<ThreadedNusMsg>,
) -> Result<bool, Box<dyn Error>> {
    let mut do_quit = false;
    // make device connection
    let device = adapter.open_device(bt_id).await?;
    adapter.connect_device(&device).await?;

    // use device to obtain service
    let nus_svc = device.discover_services_with_uuid(NUS_SVC_UUID).await?;
    let Some(nus_svc) = nus_svc.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS Service");

    // use service to obtain (RX) characteristic
    let nus_rx_chr = nus_svc
        .discover_characteristics_with_uuid(NUS_RX_CHR_UUID)
        .await?;
    let Some(nus_rx_chr) = nus_rx_chr.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS RX");

    // use service to obtain (TX) characteristic
    let nus_tx_chr = nus_svc
        .discover_characteristics_with_uuid(NUS_TX_CHR_UUID)
        .await?;
    let Some(nus_tx_chr) = nus_tx_chr.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS TX");

    // enable notifs on TX characteristic
    let mut nus_tx_notifs = nus_tx_chr.notify().await?;
    info!("enabled notifs on NUS TX");

    info!("nus chars are ready!");
    let _ = resp.send(AmConnected);

    let mut do_disconnect = false;
    loop {
        if do_quit | do_disconnect {
            break;
        }
        match device.is_connected().await {
            true => {}
            false => {
                break;
            }
        }

        // TODO: do the tokio thing where you instruct...
        // "async wait on either of these things, and action whichever comes first"

        // 1. check input if we should Disconnect -OR- relay bytes to device via nus_rx_chr
        loop {
            // match cmd.recv_timeout(Duration::from_millis(10)) {
            match timeout(Duration::from_millis(10), cmd.recv_async()).await {
                Ok(Ok(DoQuit)) => {
                    do_quit = true;
                    info!("recv DoQuit");
                    break;
                }
                Ok(Ok(DoDisconnect)) => {
                    info!("recv'd DoDisconnect");
                    do_disconnect = true;
                    break;
                }
                Ok(Ok(DataRx(rx_bytes))) => {
                    debug!("attempt send rx_bytes = {:?}", rx_bytes);
                    match nus_rx_chr.write_without_response(&rx_bytes).await {
                        Ok(_good) => {
                            info!("success send rx_bytes = {rx_bytes:?}");
                        }
                        Err(e) => {
                            error!("error send rx_bytes={rx_bytes:?} : {e}");
                        }
                    }
                }
                Ok(Ok(unh)) => {
                    warn!("unhandled msg = {unh:?}");
                }
                Ok(Err(e)) => {
                    error!("{e}");
                }
                Err(elapsed) => {
                    debug!("Timeout elapsed {elapsed}");
                    break;
                }
            }
        }

        // 2. check notifs via nus_tx_chr
        match timeout(Duration::from_millis(10), nus_tx_notifs.next()).await {
            Ok(Some(Ok(tx_bytes))) => {
                info!("success notif tx_bytes.len() = {:?}", tx_bytes.len());
                let _ = resp.send(DataTx(tx_bytes));
            }
            Ok(Some(Err(e))) => {
                error!("hmm.. error = {e}");
            }
            Ok(None) => {
                error!("hmm.. no tx bytes?");
            }
            Err(e) => {
                debug!("elapsed {e}");
            }
        }
    }

    match adapter.disconnect_device(&device).await {
        Ok(_good) => {
            info!("ok disconnect");
        }
        Err(e) => {
            error!("err disconnect {e}");
        }
    }

    Ok(do_quit)
}

/// function to spawn thread that manages BLE operations and messaging
pub fn spawn_btnus_thread(
    cmd: flume::Receiver<ThreadedNusMsg>,
    resp: egui_inbox::UiInboxSender<ThreadedNusMsg>,
) -> std::thread::JoinHandle<Option<u32>> {
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            // continually loop through....
            // idle -> scanning -> connecting -> connected -> (back to idle)
            let mut do_quit = false;
            loop {
                if do_quit {
                    break;
                }
                // NOTE: state 1a-of-4: idle (not ready)
                let mut connect_bt_id: Option<DeviceId>;
                let mut scan_map: HashMap<DeviceId, Device> = HashMap::new();
                let mut option_adapter: Option<Adapter>;
                loop {
                    // TODO: put this in an async function that returns result and use ? operator???
                    option_adapter = Adapter::default().await;
                    if option_adapter.is_none() {
                        resp.send(AmNotReady).ok();
                        std::thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                    break;
                }

                let adapter = option_adapter.unwrap(); // simplify below code
                let _ = adapter.wait_available().await;

                info!("sending AmReadyIdle(...)");
                resp.send(AmReadyIdle(format!("{:?}", &adapter))).ok();
                connect_bt_id = None;

                // NOTE: state 1b-of-4: idle (ready)
                info!("btnus waiting for {:?}", DoScanStart("".into()));
                loop {
                    match cmd.recv_async().await {
                        Ok(DoQuit) => {
                            do_quit = true;
                            break;
                        }
                        Ok(DoScanStart(_opts)) => {
                            connect_bt_id = None;
                            break;
                        }
                        Ok(DoConnect(bt_id)) => {
                            connect_bt_id = Some(bt_id);
                            break;
                        }
                        Ok(unh) => {
                            warn!("unhandled message waiting for DoScanStart(_) = {unh:?}");
                        }
                        Err(_bad) => {
                            //
                        }
                    }
                }
                if do_quit {
                    break;
                }

                // NOTE: state 2-of-4: scanning

                // NOTE: putting scan in its own scope has the effect...
                //       when the the scan stream is dropped
                //       the BT scan operations will stop
                // TODO: put this scan behavior in its own async fn
                //       this async fn could return a device_id if given a &mut (mutable reference) to cmd_recv

                // if connect_bt_id is None, then let's scan!
                if connect_bt_id.is_none() {
                    info!("starting scan");
                    let scan = adapter.scan(&[]).await;
                    if scan.is_err() {
                        resp.send(AmNotReady).ok();
                        std::thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                    let mut scan = scan.unwrap();
                    resp.send(AmScanning).ok();

                    // if scan.is
                    // match
                    info!("scan started");
                    while let Some(discovered_device) = scan.next().await {
                        // TODO: put this timeout recv in a helper for readability
                        // TODO: check if the sync method recv_timeout works just fine in here... it
                        // should...
                        match cmd.recv_timeout(Duration::from_millis(0)) {
                            Ok(DoQuit) => {
                                do_quit = true;
                                break;
                            }
                            Ok(DoScanStop) => {
                                info!("scan: recv'd DoScanStop, stopping scan");
                                break;
                            }
                            // TODO: handle connect
                            Ok(DoConnect(device_id)) => {
                                info!("scan: recv'd DoScanStop, stopping scan");
                                connect_bt_id = Some(device_id)
                            }
                            Ok(unhandled) => {
                                warn!("scan: unhandled = {unhandled:?}");
                            }
                            Err(to) => {
                                trace!("timeout waiting for msg during scan: {to}");
                                //
                            }
                        }

                        let k = discovered_device.device.id();
                        let device = discovered_device.device.clone();
                        scan_map.insert(k, device);

                        resp.send(DataScanResult(vec![discovered_device.clone()]))
                            .ok();
                    }
                    info!("scan stopped");
                } // end start-scan, i.e. if connect_bt_id.is_none()

                // NOTE: state 3-of-4: connecting
                match connect_bt_id {
                    Some(bt_id) => {
                        let _ = resp.send(AmConnecting);
                        // NOTE: state 4-of-4: connected (handled inside async fn)
                        match bt_nus_setup_and_loop(&adapter, &bt_id, &cmd, &resp).await {
                            Ok(ok_do_quit) => {
                                info!("succesful disconnect");
                                if ok_do_quit {
                                    // do_quit = true;
                                    // WARN: this *should* break the forever loop
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("bad disconnect : {e}");
                            }
                        }
                    }
                    None => {
                        // nothing to do?
                    }
                }
            } // outer forever loop
            let _ = resp.send(AmQuitted); // FIXME: check result
            info!("sent AmQuitted");
        }); // end rt.block_on ...
        None // return None to satisfy JoinHandle<Option<u32>>
    }) // returning spawned thread handle;
} // end fn spawn_btnus_thread

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // --- UUID constant tests ---

    #[test]
    fn nus_svc_uuid_has_correct_value() {
        let expected = Uuid::from_u128(0x6E400001_B5A3_F393_E0A9_E50E24DCCA9E);
        assert_eq!(NUS_SVC_UUID, expected);
    }

    #[test]
    fn nus_rx_chr_uuid_has_correct_value() {
        let expected = Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);
        assert_eq!(NUS_RX_CHR_UUID, expected);
    }

    #[test]
    fn nus_tx_chr_uuid_has_correct_value() {
        let expected = Uuid::from_u128(0x6E400003_B5A3_F393_E0A9_E50E24DCCA9E);
        assert_eq!(NUS_TX_CHR_UUID, expected);
    }

    #[test]
    fn nus_uuids_are_all_distinct() {
        assert_ne!(NUS_SVC_UUID, NUS_RX_CHR_UUID);
        assert_ne!(NUS_SVC_UUID, NUS_TX_CHR_UUID);
        assert_ne!(NUS_RX_CHR_UUID, NUS_TX_CHR_UUID);
    }

    #[test]
    fn nus_svc_uuid_matches_nordic_nus_service_spec() {
        // The NUS service UUID per Nordic spec is 6E400001-B5A3-F393-E0A9-E50E24DCCA9E
        let uuid_str = NUS_SVC_UUID.to_string().to_lowercase();
        assert_eq!(uuid_str, "6e400001-b5a3-f393-e0a9-e50e24dcca9e");
    }

    #[test]
    fn nus_rx_chr_uuid_matches_nordic_nus_rx_spec() {
        // The NUS RX characteristic UUID per Nordic spec is 6E400002-B5A3-F393-E0A9-E50E24DCCA9E
        let uuid_str = NUS_RX_CHR_UUID.to_string().to_lowercase();
        assert_eq!(uuid_str, "6e400002-b5a3-f393-e0a9-e50e24dcca9e");
    }

    #[test]
    fn nus_tx_chr_uuid_matches_nordic_nus_tx_spec() {
        // The NUS TX characteristic UUID per Nordic spec is 6E400003-B5A3-F393-E0A9-E50E24DCCA9E
        let uuid_str = NUS_TX_CHR_UUID.to_string().to_lowercase();
        assert_eq!(uuid_str, "6e400003-b5a3-f393-e0a9-e50e24dcca9e");
    }

    // --- ThreadedNusMsg trait derivation tests ---

    #[test]
    fn threaded_nus_msg_clone_works_for_simple_variants() {
        let msg = ThreadedNusMsg::AmNotReady;
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_clone_works_for_string_variant() {
        let msg = ThreadedNusMsg::AmReadyIdle("adapter_desc".to_string());
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_clone_works_for_do_scan_start() {
        let msg = ThreadedNusMsg::DoScanStart("options".to_string());
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_clone_works_for_data_tx() {
        let msg = ThreadedNusMsg::DataTx(vec![0x01, 0x02, 0x03]);
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_clone_works_for_data_rx() {
        let msg = ThreadedNusMsg::DataRx(vec![0xAA, 0xBB]);
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_partial_eq_distinguishes_variants() {
        assert_ne!(ThreadedNusMsg::AmNotReady, ThreadedNusMsg::AmConnected);
        assert_ne!(ThreadedNusMsg::AmScanning, ThreadedNusMsg::AmConnecting);
        assert_ne!(ThreadedNusMsg::DoScanStop, ThreadedNusMsg::DoDisconnect);
        assert_ne!(ThreadedNusMsg::DoQuit, ThreadedNusMsg::AmQuitted);
    }

    #[test]
    fn threaded_nus_msg_partial_eq_matches_identical_variants() {
        assert_eq!(ThreadedNusMsg::AmNotReady, ThreadedNusMsg::AmNotReady);
        assert_eq!(ThreadedNusMsg::AmConnected, ThreadedNusMsg::AmConnected);
        assert_eq!(ThreadedNusMsg::AmScanning, ThreadedNusMsg::AmScanning);
        assert_eq!(ThreadedNusMsg::AmConnecting, ThreadedNusMsg::AmConnecting);
        assert_eq!(ThreadedNusMsg::AmQuitted, ThreadedNusMsg::AmQuitted);
        assert_eq!(ThreadedNusMsg::DoScanStop, ThreadedNusMsg::DoScanStop);
        assert_eq!(ThreadedNusMsg::DoDisconnect, ThreadedNusMsg::DoDisconnect);
        assert_eq!(ThreadedNusMsg::DoQuit, ThreadedNusMsg::DoQuit);
    }

    #[test]
    fn threaded_nus_msg_partial_eq_compares_string_content() {
        let a = ThreadedNusMsg::AmReadyIdle("desc_a".to_string());
        let b = ThreadedNusMsg::AmReadyIdle("desc_b".to_string());
        let a_again = ThreadedNusMsg::AmReadyIdle("desc_a".to_string());
        assert_ne!(a, b);
        assert_eq!(a, a_again);
    }

    #[test]
    fn threaded_nus_msg_partial_eq_compares_byte_content() {
        let a = ThreadedNusMsg::DataTx(vec![1, 2, 3]);
        let b = ThreadedNusMsg::DataTx(vec![1, 2, 4]);
        let a_again = ThreadedNusMsg::DataTx(vec![1, 2, 3]);
        assert_ne!(a, b);
        assert_eq!(a, a_again);
    }

    #[test]
    fn threaded_nus_msg_debug_includes_variant_name() {
        let msg = ThreadedNusMsg::AmNotReady;
        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("AmNotReady"));
    }

    #[test]
    fn threaded_nus_msg_do_scan_start_empty_string() {
        let msg = ThreadedNusMsg::DoScanStart(String::new());
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    #[test]
    fn threaded_nus_msg_data_tx_empty_bytes() {
        let msg = ThreadedNusMsg::DataTx(vec![]);
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
        assert_eq!(cloned, ThreadedNusMsg::DataTx(vec![]));
    }

    #[test]
    fn threaded_nus_msg_data_rx_empty_bytes() {
        let msg = ThreadedNusMsg::DataRx(vec![]);
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    // --- Channel communication tests (no hardware needed) ---

    #[test]
    fn flume_channel_can_send_and_recv_threaded_nus_msg() {
        let (tx, rx) = flume::unbounded::<ThreadedNusMsg>();
        tx.send(ThreadedNusMsg::DoScanStop).unwrap();
        let received = rx.recv().unwrap();
        assert_eq!(received, ThreadedNusMsg::DoScanStop);
    }

    #[test]
    fn flume_channel_can_send_multiple_messages() {
        let (tx, rx) = flume::unbounded::<ThreadedNusMsg>();
        tx.send(ThreadedNusMsg::AmNotReady).unwrap();
        tx.send(ThreadedNusMsg::AmScanning).unwrap();
        tx.send(ThreadedNusMsg::AmConnected).unwrap();

        assert_eq!(rx.recv().unwrap(), ThreadedNusMsg::AmNotReady);
        assert_eq!(rx.recv().unwrap(), ThreadedNusMsg::AmScanning);
        assert_eq!(rx.recv().unwrap(), ThreadedNusMsg::AmConnected);
    }

    #[test]
    fn flume_channel_send_data_rx_bytes() {
        let (tx, rx) = flume::unbounded::<ThreadedNusMsg>();
        let payload = b"hello\n".to_vec();
        tx.send(ThreadedNusMsg::DataRx(payload.clone())).unwrap();
        match rx.recv().unwrap() {
            ThreadedNusMsg::DataRx(bytes) => assert_eq!(bytes, payload),
            other => panic!("Unexpected message: {:?}", other),
        }
    }
}

