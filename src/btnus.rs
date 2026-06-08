// use std::thread;

// use std::time::Duration;

use std::collections::HashMap;
// use std::error::Error as StdError;

use smlang::statemachine;

use bluest::{Adapter, AdvertisingDevice, Device, DeviceId};

use futures_lite::StreamExt;

use tokio::runtime;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};
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

    /// Scan result data
    DataScanResult(Vec<AdvertisingDevice>),

    /// NUS TX bytes (BLE notif from device)
    DataTx(Vec<u8>),

    /// NUS RX bytes (BLE write to device)
    DataRx(Vec<u8>),

    /// Command to get ready
    DoGetReady,

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
}
use ThreadedNusMsg::*;

statemachine! {
    name: Ble,
    derive_states: [Debug, Clone],
    derive_events: [Debug, Clone],
    transitions: {
        *AmNotReady + DoGetReady [try_get_ready] / success_get_ready = AmReadyIdle,

        AmReadyIdle + DoScanStart(String) [try_scan_start] / success_scan_start = AmScanning,

        AmScanning + DoScanStop [try_scan_stop] / success_scan_stop = AmReadyIdle,
        AmScanning + DoConnect(DeviceId) [try_connect_start] / success_connect_start = AmConnecting,

        AmConnecting + ConnectComplete [try_connect_finish] / success_connect_finish = AmConnected,
        AmConnecting + DoDisconnect [try_connect_fail] / success_connect_fail = AmReadyIdle,

        AmConnected + DoDisconnect [try_disconnect] / success_disconnect = AmReadyIdle,

        _ + DoQuit [try_quit] / success_quit = AmQuitted,
        AmQuitted + DoQuit = AmQuitted,
    }
}

pub struct Context {
    cmd: flume::Receiver<ThreadedNusMsg>,
    resp: egui_inbox::UiInboxSender<ThreadedNusMsg>,
    adapter: Option<Adapter>,
    connect_bt_id: Option<DeviceId>,
    scan_map: HashMap<DeviceId, Device>,
}

impl Context {
    fn new(
        cmd: flume::Receiver<ThreadedNusMsg>,
        resp: egui_inbox::UiInboxSender<ThreadedNusMsg>,
    ) -> Self {
        Self {
            cmd,
            resp,
            adapter: None,
            connect_bt_id: None,
            scan_map: HashMap::new(),
        }
    }
}

impl BleStateMachineContext for Context {
    fn try_get_ready(&self) -> Result<bool, ()> {
        Ok(self.adapter.is_some())
    }

    fn success_get_ready(&mut self) -> Result<(), ()> {
        if let Some(adapter) = &self.adapter {
            let _ = self.resp.send(AmReadyIdle(format!("{:?}", adapter)));
        }
        Ok(())
    }

    fn try_scan_start(&self, _scan_opts: &String) -> Result<bool, ()> {
        Ok(self.adapter.is_some())
    }

    fn success_scan_start(&mut self, _scan_opts: String) -> Result<(), ()> {
        let _ = self.resp.send(AmScanning);
        Ok(())
    }

    fn try_scan_stop(&self) -> Result<bool, ()> {
        Ok(true)
    }

    fn success_scan_stop(&mut self) -> Result<(), ()> {
        if let Some(adapter) = &self.adapter {
            let _ = self.resp.send(AmReadyIdle(format!("{:?}", adapter)));
        }
        Ok(())
    }

    fn try_connect_start(&self, _bt_id: &DeviceId) -> Result<bool, ()> {
        Ok(true)
    }

    fn success_connect_start(&mut self, bt_id: DeviceId) -> Result<(), ()> {
        self.connect_bt_id = Some(bt_id);
        let _ = self.resp.send(AmConnecting);
        Ok(())
    }

    fn try_connect_finish(&self) -> Result<bool, ()> {
        Ok(self.connect_bt_id.is_some())
    }

    fn success_connect_finish(&mut self) -> Result<(), ()> {
        let _ = self.resp.send(AmConnected);
        Ok(())
    }

    fn try_connect_fail(&self) -> Result<bool, ()> {
        Ok(true)
    }

    fn success_connect_fail(&mut self) -> Result<(), ()> {
        self.connect_bt_id = None;
        if let Some(adapter) = &self.adapter {
            let _ = self.resp.send(AmReadyIdle(format!("{:?}", adapter)));
        }
        Ok(())
    }

    fn try_disconnect(&self) -> Result<bool, ()> {
        Ok(true)
    }

    fn success_disconnect(&mut self) -> Result<(), ()> {
        self.connect_bt_id = None;
        if let Some(adapter) = &self.adapter {
            let _ = self.resp.send(AmReadyIdle(format!("{:?}", adapter)));
        }
        Ok(())
    }

    fn try_quit(&self) -> Result<bool, ()> {
        Ok(true)
    }

    fn success_quit(&mut self) -> Result<(), ()> {
        let _ = self.resp.send(AmQuitted);
        Ok(())
    }
}

fn msg_to_event(msg: &ThreadedNusMsg) -> Option<BleEvents> {
    match msg {
        ThreadedNusMsg::DoGetReady => Some(BleEvents::DoGetReady),
        ThreadedNusMsg::DoScanStart(opts) => Some(BleEvents::DoScanStart(opts.clone())),
        ThreadedNusMsg::DoScanStop => Some(BleEvents::DoScanStop),
        ThreadedNusMsg::DoConnect(bt_id) => Some(BleEvents::DoConnect(bt_id.clone())),
        ThreadedNusMsg::DoDisconnect => Some(BleEvents::DoDisconnect),
        ThreadedNusMsg::DoQuit => Some(BleEvents::DoQuit),
        _ => None,
    }
}

fn handle_event(
    sm: &mut BleStateMachine<Context>,
    msg: ThreadedNusMsg,
) {
    info!("recv'd: {:?}", msg);
    if let Some(event) = msg_to_event(&msg) {
        match sm.process_event(event) {
            Ok(new_state) => {
                debug!("transition to {:?}", new_state);
            }
            Err(BleError::GuardFailed(_)) => {
                warn!("guard failed for event");
            }
            Err(BleError::InvalidEvent) => {
                warn!("invalid event for current state");
            }
            Err(BleError::TransitionsFailed) | Err(BleError::ActionFailed(_)) => {
                warn!("transition/action failed");
            }
        }
    } else {
        warn!("unhandled message = {:?}", msg);
    }
}

/// async function to handle connection and active use for NUS data transfer
async fn bt_nus_setup_and_loop(
    adapter: &bluest::Adapter,
    bt_id: &DeviceId,
    cmd: &flume::Receiver<ThreadedNusMsg>,
    resp: &egui_inbox::UiInboxSender<ThreadedNusMsg>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut do_quit = false;

    // make device connection
    let device = adapter.open_device(bt_id).await?;
    adapter.connect_device(&device).await?;

    // use device to obtain service
    let svc_vec = device.discover_services_with_uuid(NUS_SVC_UUID).await?;
    let Some(nus_svc) = svc_vec.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS Service");

    // use service to obtain (RX) characteristic
    let chr_vec = nus_svc
        .discover_characteristics_with_uuid(NUS_RX_CHR_UUID)
        .await?;
    let Some(nus_rx_chr) = chr_vec.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS RX");

    // use service to obtain (TX) characteristic
    let chr_vec = nus_svc
        .discover_characteristics_with_uuid(NUS_TX_CHR_UUID)
        .await?;
    let Some(nus_tx_chr) = chr_vec.first() else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("found NUS TX");

    // enable notifs on TX characteristic
    let Ok(mut nus_tx_notifs) = nus_tx_chr.notify().await else {
        let _ = adapter.disconnect_device(&device).await?;
        return Ok(false);
    };
    info!("enabled notifs on NUS TX");
    info!("nus chars are ready!");

    loop {
        if do_quit || !device.is_connected().await {
            break;
        }

        // TODO: do the tokio thing where you instruct...
        // "async wait on either of these things, and action whichever comes first"

        // 1. check input if we should Disconnect -OR- relay bytes to device via nus_rx_chr
        // loop {
        // match cmd.recv_timeout(Duration::from_millis(10)) {
        tokio::select! {
            Ok(msg) = cmd.recv_async() => {
                match msg {
                    DoQuit => {
                        do_quit = true;
                        info!("recv DoQuit");
                        break;
                    }
                    DoDisconnect => {
                        info!("recv'd DoDisconnect");
                        break;
                    }
                    DataRx(rx_bytes) => {
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
                    unh => {
                        warn!("unhandled msg = {unh:?}");
                    }

                }
            },
            Some(Ok(tx_notif)) = nus_tx_notifs.next() => {
                debug!("sending {tx_notif:?}");
                let _ = resp.send(DataTx(tx_notif));
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.5)) => {
                debug!("Timed out!");
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
        let rt = runtime::Builder::new_multi_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();
        rt.block_on(async {
            let ctx = Context::new(cmd, resp);
            let mut sm = BleStateMachine::new(ctx);

            loop {
                match sm.state() {
                    BleStates::AmNotReady => {
                        sm.context_mut().adapter = Adapter::default().await;
                        if sm.context().adapter.is_some() {
                            let _ = sm.process_event(BleEvents::DoGetReady);
                        } else {
                            let _ = sm.context_mut().resp.send(AmNotReady);
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }

                    BleStates::AmReadyIdle => {
                        tokio::select! {
                            Ok(msg) = sm.context_mut().cmd.recv_async() => {
                                handle_event(&mut sm, msg);
                            },
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                                debug!("Timed out!");
                            }
                        }
                    }

                    BleStates::AmScanning => {
                        let (connect_bt_id, scan_map, do_scan_stop, do_quit, scan_failed) = {
                            let mut connect_bt_id: Option<DeviceId> = None;
                            let mut scan_map: HashMap<DeviceId, Device> = HashMap::new();
                            let mut do_scan_stop = false;
                            let mut do_quit = false;
                            let mut scan_failed = false;
                            
                            let scan_opt = {
                                let ctx = sm.context();
                                ctx.adapter.as_ref().unwrap().scan(&[]).await.ok()
                            };
                            
                            if let Some(mut scan) = scan_opt {
                                {
                                    let ctx = sm.context();
                                    let cmd = &ctx.cmd;
                                    let resp = &ctx.resp;

                                    loop {
                                        tokio::select! {
                                            Ok(msg) = cmd.recv_async() => {
                                                info!("recv'd: {:?}", msg);
                                                match msg {
                                                    DoQuit => {
                                                        do_quit = true;
                                                        break;
                                                    }
                                                    DoScanStop => {
                                                        info!("scan: recv'd DoScanStop, stopping scan");
                                                        do_scan_stop = true;
                                                        break;
                                                    }
                                                    DoConnect(device_id) => {
                                                        info!("scan: recv'd DoConnect, doing connect + stopping scan");
                                                        connect_bt_id = Some(device_id);
                                                        break;
                                                    }
                                                    unhandled => {
                                                        warn!("scan: unhandled = {unhandled:?}");
                                                    }
                                                }
                                            },
                                            Some(discovered_device) = scan.next() => {
                                                let k = discovered_device.device.id();
                                                let device = discovered_device.device.clone();
                                                let _ = resp.send(DataScanResult(vec![discovered_device.clone()]));
                                                scan_map.insert(k, device);
                                            },
                                            _ = tokio::time::sleep(Duration::from_secs_f32(0.5)) => {
                                                debug!("Timed out!");
                                            }
                                        }
                                    }
                                }
                            } else {
                                scan_failed = true;
                            }
                            
                            (connect_bt_id, scan_map, do_scan_stop, do_quit, scan_failed)
                        };
                        
                        sm.context_mut().scan_map = scan_map;
                        
                        if scan_failed {
                            let _ = sm.process_event(BleEvents::DoDisconnect);
                        } else if do_quit {
                            let _ = sm.process_event(BleEvents::DoQuit);
                        } else if do_scan_stop {
                            let _ = sm.process_event(BleEvents::DoScanStop);
                        } else if connect_bt_id.is_some() {
                            sm.context_mut().connect_bt_id = connect_bt_id;
                            let _ = sm.process_event(BleEvents::DoConnect(sm.context().connect_bt_id.clone().unwrap()));
                        }
                    }

                    BleStates::AmConnecting => {
                        let bt_id = sm.context().connect_bt_id.clone();
                        if let Some(bt_id) = bt_id {
                            let adapter = sm.context().adapter.as_ref().unwrap();
                            let cmd = &sm.context().cmd;
                            let resp = &sm.context().resp;
                            match bt_nus_setup_and_loop(adapter, &bt_id, cmd, resp).await {
                                Ok(ok_do_quit) => {
                                    info!("successful disconnect");
                                    if ok_do_quit {
                                        let _ = sm.process_event(BleEvents::DoQuit);
                                    } else {
                                        let _ = sm.process_event(BleEvents::ConnectComplete);
                                    }
                                }
                                Err(e) => {
                                    error!("bad disconnect : {e}");
                                    let _ = sm.process_event(BleEvents::DoDisconnect);
                                }
                            }
                        } else {
                            let _ = sm.process_event(BleEvents::DoDisconnect);
                        }
                    }

                    BleStates::AmConnected => {
                        let _ = sm.process_event(BleEvents::DoDisconnect);
                    }

                    BleStates::AmQuitted => {
                        break;
                    }
                }
            }

            info!("sent AmQuitted");
        });
        None
    })
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
