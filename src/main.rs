#![crate_name = "nusgui"]

use core::f32;
use std::collections::HashMap;

use bluest::AdvertisingDevice;
use bluest::DeviceId;
use eframe::egui::{Button, TopBottomPanel, Vec2};
use eframe::{App, CreationContext, Frame, egui, epaint::Color32};
use egui::RichText;
use egui_colors::Colorix;
// use egui_colors::{Colorix; ThemeColor};
use egui::text::{CCursor, CCursorRange};
use egui::{Align, CentralPanel, Context, Layout, ThemePreference, Ui};
use egui_extras::Column;
use egui_inbox::UiInbox;
use egui_selectable_table::SelectableTable;

// use serde;

mod btnus;
mod scan_table;

use btnus::ThreadedNusMsg::*;
use btnus::spawn_btnus_thread;

use tracing::metadata::LevelFilter;
use tracing_subscriber::filter;
use tracing_subscriber::prelude::*;

use tracing::{error, info, warn};

use crate::btnus::ThreadedNusMsg;
use crate::scan_table::{ScanColumns, ScanConfig, ScanRow};

use strum::IntoEnumIterator;
// 0.25
//
use flume::Sender;

/// core struct representing the state of nusgui
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // Use Default if fields are skipped
struct NusGui {
    /// (flume) Sender for gui->bt send comms
    #[serde(skip)]
    cmd_tx: Sender<ThreadedNusMsg>,

    /// (egui_inbox) inbox (like Receiver) for gui<-bt recv comms
    #[serde(skip)]
    inbox: UiInbox<ThreadedNusMsg>,

    /// message passed state of gui+bt threads
    #[serde(skip)]
    bt_state: ThreadedNusMsg,

    /// bt thread handle
    // #[serde(skip)]
    // _bt_handle: std::thread::JoinHandle<Option<u32>>,

    /// vector of AdvertisingDevice objects from scan process
    #[serde(skip)]
    scan_vec: Vec<AdvertisingDevice>,

    /// hashmap of AdvertisingDevice objects from scan process
    #[serde(skip)]
    scan_map: HashMap<DeviceId, AdvertisingDevice>,

    /// scan results table
    #[serde(skip)]
    table: SelectableTable<ScanRow, ScanColumns, ScanConfig>,

    /// actual nus string data stored as single multiline string
    nus_tx_multi_string: String,

    /// text input string from text input field
    nus_rx_single_string: String,

    /// actual nus string from text input field
    nus_rx_history: Vec<String>,

    /// position in input history lookup
    nus_rx_history_index: Option<usize>,

    /// temporary state variable to 'snap the cursor' when updating input field from history
    nus_rx_snap_cursor: bool,

    /// Only show devices advertising NUS service in scan table
    scan_filt_svc_present: bool,

    /// Only show devices with this string in the advertised name
    scan_filt_name: String,

    /// boolean latch to quit the application
    do_quit: bool,

    /// boolean latch to run some code (1) time at init
    latch_once: bool,

    /// color theme helper object
    #[serde(skip)]
    colorix: Colorix,
}

const PEACH32: Color32 = Color32::from_rgb(0xFF, 0xD3, 0xAC);

impl NusGui {
    /// Create new NusGui instance
    ///
    /// This function
    /// 1. Creates some empty/default objects to satisfy fields
    /// 2. Creates 'both sides' of a `flume` channel for sending commands
    ///   * `cmd_tx: flume::Sender<ThreadedNusMsg>` is stored in NusGui for use by the GUI thread
    ///   * `cmd_rx: flume::Receiver<ThreadedNusMsg>` is given to spawned BT thread
    /// 3. Creates 'both sides' of an `egui_inbox` 'channel'
    ///   * `inbox: egui_inbox::UiInbox<ThreadedNusMsg>` is stored in NusGui for GUI thread to receive messages from BT thread
    ///   * `sender: egui_inbox::UiInboxSender<ThreadedNusMsg>` is given to BT thread to send messages
    /// 4. Spawns BT thread and saves the (currently unused) thread handle
    /// 5. Returns a new instance of `NusGui` that is attached to a spawned BT thread
    pub fn new(ctx: &Context, cc: &CreationContext) -> Self {
        cc.egui_ctx
            .options_mut(|a| a.theme_preference = ThemePreference::System);

        // Otherwise, return default
        let mut out: NusGui = Default::default();
        out.colorix = Colorix::global(ctx, egui_colors::utils::EGUI_THEME);

        // load important fields manually for now
        if let Some(some_storage) = cc.storage {
            if let Some(some_scan_filt_name) = some_storage.get_string("scan_filt_name") {
                out.scan_filt_name = some_scan_filt_name
                    .replace("\"", "")
                    .replace("'", "")
                    .replace("\\", "");
            }
            if let Some(some_scan_filt_svc_present) =
                some_storage.get_string("scan_filt_svc_present")
            {
                match some_scan_filt_svc_present.parse::<bool>() {
                    Ok(ok_bool) => {
                        out.scan_filt_svc_present = ok_bool;
                    }
                    Err(e) => {
                        error!("{e}");
                    }
                }
                // match
            }
        }

        // if let Some(storage) = cc.storage {
        //     if let Some(saved_state) = eframe::get_value(storage, eframe::APP_KEY) {
        //         // let stored =
        //         // TODO: how to 'overlay' fields from 'stored' on-top-of 'out'
        //     }
        // }
        //
        out
    }

    /// This function handles up + down arrow key presses to satisfy navigating text input history
    fn process_input_history(&mut self, ui: &mut Ui) {
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) {
                self.nus_rx_snap_cursor = true;
                if let Some(index) = self.nus_rx_history_index {
                    if index > 0 {
                        self.nus_rx_history_index = Some(index - 1);
                        self.nus_rx_single_string
                            .clone_from(&self.nus_rx_history[index - 1]);
                    }
                } else if !self.nus_rx_history.is_empty() {
                    // Start navigating from the last entry
                    let index = self.nus_rx_history.len() - 1;
                    self.nus_rx_history_index = Some(index);
                    self.nus_rx_single_string
                        .clone_from(&self.nus_rx_history[index]);
                }
            } else if i.key_pressed(egui::Key::ArrowDown) {
                self.nus_rx_snap_cursor = true;
                if let Some(index) = self.nus_rx_history_index {
                    if index < self.nus_rx_history.len() - 1 {
                        self.nus_rx_history_index = Some(index + 1);
                        self.nus_rx_single_string
                            .clone_from(&self.nus_rx_history[index + 1]);
                    } else {
                        // Reached the end of history, clear input
                        self.nus_rx_history_index = None;
                        self.nus_rx_single_string.clear();
                    }
                }
            }
        });
    }

    /// Read and process incoming messages to `egui_inbox::UiInbox<ThreadedNusMsg>`
    fn process_inbox(&mut self, _ctx: &Context, ui: &mut Ui) {
        // loop through all received responses
        for response in self.inbox.read(ui) {
            let m = response.clone();
            // let m = response;
            match m {
                AmQuitted => {
                    self.do_quit = true;
                }
                AmReadyIdle(adapter_desc) => {
                    self.bt_state = AmReadyIdle(adapter_desc.clone());
                }
                AmNotReady | AmScanning | AmConnecting | AmConnected => {
                    self.bt_state = m;
                }
                DataTx(nus_tx_bytes) => {
                    // TODO: implement a more robust strategy for handling utf8 errors...
                    //       using lossy function is okay for now
                    let tmp_str = String::from_utf8_lossy(&nus_tx_bytes);
                    self.nus_tx_multi_string.push_str(&tmp_str);

                    // TODO: add incremental file logging here
                }
                DataScanResult(recvd_scans) => {
                    // scan_vec.extend(new_scans);
                    for adv_dev in recvd_scans {
                        let id = adv_dev.device.id();
                        let unique = !self.scan_map.contains_key(&id);
                        if unique && self.scan_filt_match(&adv_dev) {
                            self.scan_map.insert(id, adv_dev.clone());
                            self.scan_vec.push(adv_dev);
                            for scan_obj in &self.scan_vec {
                                self.table.add_modify_row(|rows| {
                                    // edit row here
                                    for r in rows {
                                        if r.1.row_data.bt_id.eq(&Some(scan_obj.device.id())) {
                                            // copy the just-received thread_infos the correct table row correct
                                            // table row data
                                            r.1.row_data = scan_obj_to_scan_row(scan_obj);
                                            return None; // indicate we modified a row, don't add a new one
                                        }
                                    }

                                    // indicate we didn't find a row to modify, so add this data as a new row
                                    let scan_row = scan_obj_to_scan_row(scan_obj);
                                    Some(scan_row)
                                });
                            }
                            self.table.recreate_rows();
                        }
                    }
                }
                unhandled => {
                    warn!("unhandled msg = {unhandled:?}");
                }
            }
        }
    } // end process_inbox

    /// convenience function to draw top panel of GUI
    fn draw_top_panel(&mut self, _ctx: &Context, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // egui::widgets::global_theme_preference_buttons(ui);
            self.colorix.light_dark_toggle_button(ui, 30.0);
            ui.add_space(10.);
            self.colorix.themes_dropdown(ui, None, false);

            let discon_button = Button::new(
                // Use RichText to customize the text color
                egui::RichText::new("Disconnect").color(Color32::BLACK), // Set the text color to white
            )
            // Use the fill method to set the button's background color to red
            .fill(PEACH32); //

            if AmConnected == self.bt_state && ui.add(discon_button).clicked() {
                let _ = self.cmd_tx.send(DoDisconnect);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let quit_button = Button::new(
                    // Use RichText to customize the text color
                    egui::RichText::new("QUIT").color(Color32::WHITE), // Set the text color to white
                )
                // Use the fill method to set the button's background color to red
                .fill(Color32::DARK_RED); //
                //
                if ui.add(quit_button).clicked() {
                    let _ = self.cmd_tx.send(DoQuit);
                    // self.bt_state = AmQuitting;
                    // FIXME: clean up disconnect+quit logic to ensure actual BT disconnect
                    // ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    /// convenience function to draw central panel of GUI
    fn draw_central_panel(&mut self, _ctx: &Context, ui: &mut Ui) {
        // make 'actionable copy' of bt_state so called functions can alter self.bt_state
        // let bt_state = self.bt_state.clone();
        match self.bt_state.clone() {
            AmNotReady => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Waiting for BT hardware...");
                });
                ui.label("Is there BT hardware connected?");
            }
            AmReadyIdle(_adapter_name) => {
                self.draw_central_panel_idle_scan(ui);
            }
            AmScanning => {
                self.draw_central_panel_idle_scan(ui);
            }
            AmConnecting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Waiting for BT connection...");
                });
            }
            AmConnected => {
                self.draw_central_panel_connected(ui);
            }
            unhandled => {
                ui.label(format!(
                    "Well, this is awkward, I didn't expect to be in a state of {:?}",
                    unhandled
                ));
            }
        }
    }

    /// convenience function to draw central panel of GUI when idle or scanning
    fn draw_central_panel_idle_scan(&mut self, ui: &mut Ui) {
        ui.label(format!("State: {:?}", self.bt_state));
        ui.label(format!("Found {} devices", self.scan_map.len()));

        ui.separator();
        ui.label(RichText::new("Scan Filters").heading());
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.scan_filt_svc_present, "NUS Service");
            ui.label("Name: ");
            ui.text_edit_singleline(&mut self.scan_filt_name);
        });
        ui.separator();

        // scan start/stop
        ui.horizontal(|ui| {
            let start_enabled = matches!(self.bt_state, AmReadyIdle(_));
            let stop_enabled = matches!(self.bt_state, AmScanning);
            ui.add_enabled_ui(start_enabled, |ui| {
                if ui.button("Start Scan").clicked() {
                    // TODO: provide actual scan options into DoScanStart message from UI
                    let send_start_scan_res = self.cmd_tx.send(DoScanStart("".into()));
                    info!("send_start_scan_res = {send_start_scan_res:?}");
                }
            });
            ui.add_enabled_ui(stop_enabled, |ui| {
                if ui.button("Stop Scan").clicked() {
                    let send_stop_scan_res = self.cmd_tx.send(DoScanStop);
                    info!("send_stop_scan_res = {send_stop_scan_res:?}");
                }
            });
            if ui.button("Clear Scan List").clicked() {
                self.scan_map.clear();
                self.scan_vec.clear();
                self.table.clear_all_rows();
                self.table.recreate_rows();
            }
        });

        let work_row_id = self.table.config.connect_row_id;
        if let Some(connect_row_id) = work_row_id {
            // latch variable out
            self.table.config.connect_row_id = None;

            let row_map = self.table.get_all_rows();
            match row_map.get(&connect_row_id) {
                Some(connect_row) => {
                    let row_data = connect_row.row_data.clone();
                    info!(
                        "Should connect to name={}, rssi={}, bt_id={:?}", //
                        row_data.name, row_data.rssi, row_data.bt_id
                    );

                    match row_data.bt_id {
                        Some(bt_id) => {
                            self.nus_tx_multi_string.clear();
                            let _ = self.cmd_tx.send(DoScanStop);
                            let _ = self.cmd_tx.send(DoConnect(bt_id));
                            self.bt_state = AmConnecting;
                        }
                        None => {
                            warn!("Couldn't get bt_id from select/connect row_data");
                        }
                    }
                }
                None => {
                    warn!("Couldn't resolve row id {connect_row_id}");
                }
            }
            return;
        }

        self.table.show_ui(ui, |table| {
            let mut table = table
                .drag_to_scroll(false)
                .striped(true)
                .resizable(true)
                .cell_layout(Layout::left_to_right(Align::Center))
                .auto_shrink([false; 2])
                .min_scrolled_height(0.0);

            for _col in ScanColumns::iter() {
                // table = table.column(Column::initial(150.0))
                table = table.column(Column::auto())
            }
            table
        });
    } // end draw_central_panel

    /// convenience function to draw central panel of GUI when connected to a NUS capable device
    fn draw_central_panel_connected(&mut self, ui: &mut Ui) {
        // TODO: add multiline text edit via ui.enabled(false) w/ diff. APIs
        //       reason: adding .interactive(false) to multiline TextEdit makes the text
        //       unselectable and uncopy-able
        let text_color = ui.visuals().text_color();
        egui::ScrollArea::both()
            .auto_shrink(false)
            .max_height(ui.available_height() - 30.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_enabled(
                    true,
                    egui::TextEdit::multiline(&mut self.nus_tx_multi_string.to_owned())
                        .font(egui::TextStyle::Monospace) // Monospace for terminal look
                        .desired_width(f32::INFINITY)
                        // .min_size(Vec2::new(ui.available_width(), ui.available_height()))
                        .min_size(ui.available_size())
                        .interactive(true)
                        .frame(true)
                        .text_color(text_color), // .text_color(egui::Color32::from_rgb(0xDD, 0xDD, 0xDD)),
                                                 // .show(ui);
                );
            });

        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Send: ").monospace());
                let mut nus_rx_line_edit =
                    egui::TextEdit::singleline(&mut self.nus_rx_single_string)
                        .font(egui::TextStyle::Monospace) // Monospace for terminal look
                        .desired_width(f32::INFINITY)
                        .min_size(Vec2::new(200.0, 25.0)) // Set min width/height
                        .show(ui);
                // if self.nus_rx_snap_cursor {
                //     nus_rx_line_edit.mo
                //
                // }
                // let input = ui.text_edit_singleline(&mut self.nus_rx_single_string);
                let nus_rx_line_input = nus_rx_line_edit.response;
                if self.nus_rx_snap_cursor {
                    self.nus_rx_snap_cursor = false;
                    // Create a new selection range with both start and end at the text length
                    let cursor_pos = self.nus_rx_single_string.len();
                    let min = CCursor::new(cursor_pos);
                    let max = CCursor::new(cursor_pos);
                    let new_range = CCursorRange::two(min, max);

                    // Update the state
                    nus_rx_line_edit
                        .state
                        .cursor
                        .set_char_range(Some(new_range));
                    nus_rx_line_edit.state.store(ui.ctx(), nus_rx_line_input.id);
                }

                if nus_rx_line_input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // use trimmed string for history + debug logging
                    let rx_string = self.nus_rx_single_string.trim().to_string(); // clone+trim
                    self.nus_rx_history.push(rx_string.clone());
                    info!("Sending (with bytes): DataRx({})", rx_string);

                    // add newline for actually sending
                    let rx_string = format!("{rx_string}\n");
                    let rx_bytes = rx_string.into_bytes();
                    let _ = self.cmd_tx.send(DataRx(rx_bytes)); // FIXME: check result
                    self.nus_rx_single_string.clear();
                    nus_rx_line_input.request_focus();

                    // reset history index...
                    self.nus_rx_history_index = None;
                }

                // handle input field history after we maybe just added something
                self.process_input_history(ui);
            });
        });
    }

    fn scan_filt_match(&self, adv_dev: &AdvertisingDevice) -> bool {
        scan_filt_check(
            self.scan_filt_svc_present,
            &self.scan_filt_name,
            &adv_dev.adv_data.services,
            adv_dev.adv_data.local_name.as_deref(),
        )
    }
}

/// Pure filter logic extracted for testability.
///
/// Returns `true` if the device represented by the given advertisement data
/// passes all active scan filters.
fn scan_filt_check(
    filt_svc_present: bool,
    filt_name: &str,
    adv_services: &[uuid::Uuid],
    adv_local_name: Option<&str>,
) -> bool {
    // check 0: if no filter is active, every device passes
    if !filt_svc_present && filt_name.trim().is_empty() {
        return true;
    }

    // check 1: NUS service UUID must be in the advertised service list
    if filt_svc_present && !adv_services.contains(&btnus::NUS_SVC_UUID) {
        return false;
    }

    // check 2: advertised name must contain the filter string
    if !filt_name.is_empty() {
        match adv_local_name {
            Some(adv_dev_name) => {
                if !adv_dev_name.contains(filt_name) {
                    return false;
                }
            }
            None => {
                return false;
            }
        }
    }

    true
}

impl Default for NusGui {
    fn default() -> Self {
        // NOTE: async/thread comms
        let bt_state: ThreadedNusMsg = AmNotReady;
        let scan_vec: Vec<AdvertisingDevice> = vec![];
        let scan_map: HashMap<DeviceId, AdvertisingDevice> = HashMap::default();

        let scan_columns = ScanColumns::iter().collect();
        // Auto reload after each 10k table row add or modification
        let table = SelectableTable::new(scan_columns)
            .auto_reload(10_000)
            .auto_scroll()
            .horizontal_scroll()
            .select_full_row()
            .no_ctrl_a_capture();

        let (cmd_tx, cmd_rx) = flume::unbounded();
        let inbox: UiInbox<ThreadedNusMsg> = UiInbox::new();
        let resp_tx = inbox.sender();

        // NOTE: spawn btnus thread with async runtime
        let _bt_handle: std::thread::JoinHandle<Option<u32>> = spawn_btnus_thread(cmd_rx, resp_tx);

        let nus_tx_multi_string: String = "".into();
        let nus_rx_single_string: String = "".into();
        let nus_rx_history = vec![];
        let nus_rx_history_index = None;
        let nus_rx_snap_cursor = false;

        // let colorix = Colorix::global(ctx, egui_colors::utils::EGUI_THEME);
        let colorix = Colorix::default();

        Self {
            cmd_tx,
            inbox,
            bt_state,
            // _bt_handle,
            scan_vec,
            scan_map,
            // scan_columns,
            table,
            nus_tx_multi_string,
            nus_rx_single_string,
            nus_rx_history,
            nus_rx_history_index,
            nus_rx_snap_cursor,
            //
            scan_filt_svc_present: false,
            scan_filt_name: "".into(),
            //
            do_quit: false,
            latch_once: true,
            colorix,
        }
    }
}

impl App for NusGui {
    // Saved state is loaded here at startup
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        eframe::set_value(storage, "scan_filt_name", &self.scan_filt_name);
        eframe::set_value(
            storage,
            "scan_filt_svc_present",
            &self.scan_filt_svc_present,
        );
    }
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if self.latch_once {
            self.latch_once = false;
            // ??
        }

        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.draw_top_panel(ctx, ui);
        });

        CentralPanel::default().show(ctx, |ui| {
            // WARN: this call to process_inbox has to be *somewhere* but...
            // the current function calls in process_inbox require ui: &Ui, but ....
            // only because ui: &Ui has a reference to the ctx: &Context
            // TODO: use different function calls and move call of process_inbox to outside the
            // CentralPanel clause in this function; it would be cleaner at the root of this update
            // function

            // NOTE: update data from received messages in inbox
            self.process_inbox(ctx, ui);

            if self.do_quit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }

            // draw central panel
            self.draw_central_panel(ctx, ui);
        });
    }
}

pub fn main() -> eframe::Result<()> {
    // NOTE: logging/tracing config first
    let filter = filter::Targets::new()
        // Enable the `INFO` level for anything in `my_crate`
        .with_default(LevelFilter::INFO)
        .with_target("nusgui", LevelFilter::INFO)
        .with_target("bluest", LevelFilter::WARN);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]), // Set default size here
        ..Default::default()
    };

    eframe::run_native(
        "NUS GUI",
        options,
        Box::new(|cc| {
            Ok(Box::new(
                //
                NusGui::new(&cc.egui_ctx.clone(), cc), //
            ))
        }), //
    )
}

fn scan_obj_to_scan_row(scan_obj: &AdvertisingDevice) -> ScanRow {
    ScanRow {
        bt_id: Some(scan_obj.device.id()),
        name: scan_obj.device.name().unwrap_or("n/a".into()),
        rssi: scan_obj.rssi.unwrap_or(-200_i16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use btnus::NUS_SVC_UUID;
    use uuid::Uuid;

    // Helper: a UUID that is NOT the NUS service UUID
    fn other_uuid() -> Uuid {
        Uuid::from_u128(0x12345678_1234_1234_1234_123456789ABC)
    }

    // --- scan_filt_check: no filters active ---

    #[test]
    fn no_filters_matches_device_with_no_services_and_no_name() {
        assert!(scan_filt_check(false, "", &[], None));
    }

    #[test]
    fn no_filters_matches_device_with_nus_service() {
        assert!(scan_filt_check(
            false,
            "",
            &[NUS_SVC_UUID],
            Some("MyDevice")
        ));
    }

    #[test]
    fn no_filters_matches_device_with_other_service() {
        assert!(scan_filt_check(false, "", &[other_uuid()], None));
    }

    #[test]
    fn whitespace_only_name_filter_treated_as_no_filter() {
        // filt_name that is only whitespace → trim().is_empty() is true → no filter
        assert!(scan_filt_check(false, "   ", &[], None));
        assert!(scan_filt_check(false, "\t\n", &[], None));
    }

    // --- scan_filt_check: NUS service filter only ---

    #[test]
    fn svc_filter_accepts_device_advertising_nus_service() {
        assert!(scan_filt_check(true, "", &[NUS_SVC_UUID], None));
    }

    #[test]
    fn svc_filter_rejects_device_with_no_services() {
        assert!(!scan_filt_check(true, "", &[], None));
    }

    #[test]
    fn svc_filter_rejects_device_with_only_other_services() {
        assert!(!scan_filt_check(true, "", &[other_uuid()], None));
    }

    #[test]
    fn svc_filter_accepts_device_with_nus_service_among_others() {
        assert!(scan_filt_check(
            true,
            "",
            &[other_uuid(), NUS_SVC_UUID],
            None
        ));
    }

    // --- scan_filt_check: name filter only ---

    #[test]
    fn name_filter_accepts_exact_match() {
        assert!(scan_filt_check(
            false,
            "NordicDevice",
            &[],
            Some("NordicDevice")
        ));
    }

    #[test]
    fn name_filter_accepts_substring_match() {
        assert!(scan_filt_check(false, "Nordic", &[], Some("NordicDevice")));
    }

    #[test]
    fn name_filter_accepts_suffix_match() {
        assert!(scan_filt_check(false, "Device", &[], Some("NordicDevice")));
    }

    #[test]
    fn name_filter_rejects_non_matching_name() {
        assert!(!scan_filt_check(false, "Nordic", &[], Some("AcmeDevice")));
    }

    #[test]
    fn name_filter_rejects_device_with_no_advertised_name() {
        assert!(!scan_filt_check(false, "Nordic", &[], None));
    }

    #[test]
    fn name_filter_is_case_sensitive() {
        // "nordic" should NOT match "NordicDevice" because filter is case-sensitive
        assert!(!scan_filt_check(false, "nordic", &[], Some("NordicDevice")));
    }

    #[test]
    fn name_filter_accepts_empty_advertised_name_when_filter_is_empty() {
        // An empty filter means no name filter is active → device passes
        assert!(scan_filt_check(false, "", &[], Some("")));
    }

    // --- scan_filt_check: both filters active ---

    #[test]
    fn both_filters_accepts_device_meeting_both_criteria() {
        assert!(scan_filt_check(
            true,
            "Nordic",
            &[NUS_SVC_UUID],
            Some("NordicShell")
        ));
    }

    #[test]
    fn both_filters_rejects_device_missing_nus_service_but_matching_name() {
        assert!(!scan_filt_check(
            true,
            "Nordic",
            &[other_uuid()],
            Some("NordicShell")
        ));
    }

    #[test]
    fn both_filters_rejects_device_with_nus_service_but_wrong_name() {
        assert!(!scan_filt_check(
            true,
            "Nordic",
            &[NUS_SVC_UUID],
            Some("AcmeDevice")
        ));
    }

    #[test]
    fn both_filters_rejects_device_with_nus_service_but_no_name() {
        assert!(!scan_filt_check(true, "Nordic", &[NUS_SVC_UUID], None));
    }

    #[test]
    fn both_filters_rejects_device_with_no_services_and_wrong_name() {
        assert!(!scan_filt_check(true, "Nordic", &[], Some("AcmeDevice")));
    }

    // --- scan_filt_check: boundary / regression cases ---

    #[test]
    fn name_filter_empty_string_with_svc_filter_active_accepts_nus_device() {
        // name filter is empty (no-op) + svc filter active → only check service
        assert!(scan_filt_check(true, "", &[NUS_SVC_UUID], None));
    }

    #[test]
    fn name_filter_matches_full_name_in_multi_service_list() {
        assert!(scan_filt_check(
            false,
            "Shell",
            &[other_uuid(), NUS_SVC_UUID],
            Some("BluetoothShellDevice")
        ));
    }

    #[test]
    fn svc_filter_on_but_name_filter_empty_rejects_device_without_nus() {
        assert!(!scan_filt_check(true, "", &[], Some("NordicDevice")));
    }

    #[test]
    fn name_filter_nonempty_but_svc_filter_off_accepts_device_with_matching_name_and_no_services() {
        // name matches, no services required since svc filter is off
        assert!(scan_filt_check(false, "XIAO", &[], Some("XIAO_BLE")));
    }

    #[test]
    fn name_filter_exact_single_char_match() {
        assert!(scan_filt_check(false, "X", &[], Some("XIAO")));
    }

    #[test]
    fn name_filter_longer_than_advertised_name_rejects() {
        assert!(!scan_filt_check(
            false,
            "VeryLongFilterString",
            &[],
            Some("Short")
        ));
    }
}

