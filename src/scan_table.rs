use bluest::DeviceId;
use egui_selectable_table::{
    ColumnOperations, ColumnOrdering, SelectableRow, SelectableTable, SortOrder,
};

use strum_macros::{Display, EnumIter}; // 0.25

use egui::{Button, Ui};

#[derive(Default, Clone)]
/// Scan Table Config + State
pub struct ScanConfig {
    /// device that should be connected to when clicked in scan table
    pub connect_row_id: Option<i64>,
    pub _connect_dev_id: Option<DeviceId>,
}

#[derive(Clone, Default)]
/// scan table row data
pub struct ScanRow {
    /// Bluetooth DeviceId
    pub bt_id: Option<DeviceId>,
    /// Name of Bluetooth Device
    pub name: String,
    /// Received Signal Strength Indicator
    pub rssi: i16,
}

#[derive(Eq, PartialEq, Debug, Ord, PartialOrd, Clone, Copy, Hash, Default, EnumIter, Display)]
/// visible columns of scan table
pub enum ScanColumns {
    #[default]
    #[strum(to_string = "Name")]
    Name,
    #[strum(to_string = "RSSI")]
    RSSI,
}

impl ColumnOperations<ScanRow, ScanColumns, ScanConfig> for ScanColumns {
    fn column_text(&self, row: &ScanRow) -> String {
        match self {
            // ScanColumns::Id => row.id.to_string(),
            // ScanColumns::Id => "".into(), // row.id.to_string(),
            // ScanColumns::Address => row.addr.to_string(),
            ScanColumns::Name => row.name.to_string(),
            ScanColumns::RSSI => row.rssi.to_string(),
        }
    }
    fn create_header(
        &self,
        ui: &mut Ui,
        sort_order: Option<SortOrder>,
        _table: &mut SelectableTable<ScanRow, ScanColumns, ScanConfig>,
    ) -> Option<egui::Response> {
        let mut text = self.to_string();

        if let Some(sort) = sort_order {
            match sort {
                SortOrder::Descending => text += "🔽",
                SortOrder::Ascending => text += "🔼",
            }
        }
        let selected = sort_order.is_some();
        let resp = ui.add_sized(ui.available_size(), Button::selectable(selected, text));
        Some(resp)
    }
    fn create_table_row(
        &self,
        ui: &mut Ui,
        row: &SelectableRow<ScanRow, ScanColumns>,
        cell_selected: bool,
        table: &mut SelectableTable<ScanRow, ScanColumns, ScanConfig>,
    ) -> egui::Response {
        let _row_id = row.id;
        let row_data = &row.row_data;
        let _config = table.config.clone();

        let text = match self {
            // ScanColumns::Id => format!("{row_id}"), //row_data.id.to_string(),
            // ScanColumns::Address => row_data.addr.to_string(),
            ScanColumns::Name => row_data.name.to_string(),
            ScanColumns::RSSI => row_data.rssi.to_string(),
        };

        // The same approach works for both cell based selection and for entire row selection on
        // drag.
        let resp = ui.add_sized(ui.available_size(), Button::selectable(cell_selected, text));

        if resp.clicked() {
            table.config.connect_row_id = Some(row.id);
        }

        // resp.context_menu(|ui| {
        //     if ui.button("Connect").clicked() {
        //         table.config.connect_row_id = Some(row.id);
        //     }
        // });
        resp
    }
}

impl ColumnOrdering<ScanRow> for ScanColumns {
    fn order_by(&self, row_1: &ScanRow, row_2: &ScanRow) -> std::cmp::Ordering {
        match self {
            // ScanColumns::Id => row_1.id.cmp(&row_2.id),
            // ScanColumns::Address => row_1.addr.cmp(&row_2.addr),
            ScanColumns::Name => row_1.name.cmp(&row_2.name),
            ScanColumns::RSSI => row_1.rssi.cmp(&row_2.rssi),
        }
    }
}
