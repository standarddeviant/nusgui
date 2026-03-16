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
    Rssi,
}

impl ColumnOperations<ScanRow, ScanColumns, ScanConfig> for ScanColumns {
    fn column_text(&self, row: &ScanRow) -> String {
        match self {
            // ScanColumns::Id => row.id.to_string(),
            // ScanColumns::Id => "".into(), // row.id.to_string(),
            // ScanColumns::Address => row.addr.to_string(),
            ScanColumns::Name => row.name.to_string(),
            ScanColumns::Rssi => row.rssi.to_string(),
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
            ScanColumns::Rssi => row_data.rssi.to_string(),
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
            ScanColumns::Rssi => row_1.rssi.cmp(&row_2.rssi),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    // --- ScanRow tests ---

    #[test]
    fn scan_row_default_has_none_bt_id() {
        let row = ScanRow::default();
        assert!(row.bt_id.is_none());
    }

    #[test]
    fn scan_row_default_has_empty_name() {
        let row = ScanRow::default();
        assert_eq!(row.name, "");
    }

    #[test]
    fn scan_row_default_has_zero_rssi() {
        let row = ScanRow::default();
        assert_eq!(row.rssi, 0_i16);
    }

    #[test]
    fn scan_row_fields_are_settable() {
        let row = ScanRow {
            bt_id: None,
            name: "TestDevice".to_string(),
            rssi: -75_i16,
        };
        assert_eq!(row.name, "TestDevice");
        assert_eq!(row.rssi, -75_i16);
    }

    #[test]
    fn scan_row_clone_produces_equal_row() {
        let row = ScanRow {
            bt_id: None,
            name: "CloneMe".to_string(),
            rssi: -80_i16,
        };
        let cloned = row.clone();
        assert_eq!(cloned.name, row.name);
        assert_eq!(cloned.rssi, row.rssi);
    }

    // --- ScanConfig tests ---

    #[test]
    fn scan_config_default_has_none_connect_row_id() {
        let config = ScanConfig::default();
        assert!(config.connect_row_id.is_none());
    }

    #[test]
    fn scan_config_default_has_none_connect_dev_id() {
        let config = ScanConfig::default();
        assert!(config._connect_dev_id.is_none());
    }

    #[test]
    fn scan_config_connect_row_id_can_be_set() {
        let mut config = ScanConfig::default();
        config.connect_row_id = Some(42_i64);
        assert_eq!(config.connect_row_id, Some(42_i64));
    }

    #[test]
    fn scan_config_connect_row_id_can_be_cleared() {
        let mut config = ScanConfig::default();
        config.connect_row_id = Some(1_i64);
        config.connect_row_id = None;
        assert!(config.connect_row_id.is_none());
    }

    #[test]
    fn scan_config_clone_preserves_fields() {
        let mut config = ScanConfig::default();
        config.connect_row_id = Some(7_i64);
        let cloned = config.clone();
        assert_eq!(cloned.connect_row_id, Some(7_i64));
    }

    // --- ScanColumns display (strum) tests ---

    #[test]
    fn scan_columns_name_displays_as_name() {
        // ScanColumns::Name has #[strum(to_string = "Name")]
        assert_eq!(ScanColumns::Name.to_string(), "Name");
    }

    #[test]
    fn scan_columns_rssi_displays_as_rssi() {
        // ScanColumns::Rssi has #[strum(to_string = "RSSI")] - note the strum label is "RSSI"
        assert_eq!(ScanColumns::Rssi.to_string(), "RSSI");
    }

    #[test]
    fn scan_columns_default_is_name() {
        let default_col: ScanColumns = Default::default();
        assert_eq!(default_col, ScanColumns::Name);
    }

    #[test]
    fn scan_columns_partial_eq_same_variant() {
        assert_eq!(ScanColumns::Name, ScanColumns::Name);
        assert_eq!(ScanColumns::Rssi, ScanColumns::Rssi);
    }

    #[test]
    fn scan_columns_partial_eq_different_variants() {
        assert_ne!(ScanColumns::Name, ScanColumns::Rssi);
    }

    // --- ScanColumns column_text tests ---

    #[test]
    fn column_text_name_returns_row_name() {
        let row = ScanRow {
            bt_id: None,
            name: "DeviceFoo".to_string(),
            rssi: -60_i16,
        };
        let text = ScanColumns::Name.column_text(&row);
        assert_eq!(text, "DeviceFoo");
    }

    #[test]
    fn column_text_rssi_returns_row_rssi_as_string() {
        let row = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: -95_i16,
        };
        let text = ScanColumns::Rssi.column_text(&row);
        assert_eq!(text, "-95");
    }

    #[test]
    fn column_text_rssi_handles_positive_value() {
        let row = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: 10_i16,
        };
        let text = ScanColumns::Rssi.column_text(&row);
        assert_eq!(text, "10");
    }

    #[test]
    fn column_text_rssi_handles_zero() {
        let row = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: 0_i16,
        };
        let text = ScanColumns::Rssi.column_text(&row);
        assert_eq!(text, "0");
    }

    #[test]
    fn column_text_name_handles_empty_name() {
        let row = ScanRow::default();
        let text = ScanColumns::Name.column_text(&row);
        assert_eq!(text, "");
    }

    // --- ColumnOrdering tests ---

    #[test]
    fn order_by_name_sorts_alphabetically_ascending() {
        let row_a = ScanRow {
            bt_id: None,
            name: "Alpha".to_string(),
            rssi: 0,
        };
        let row_b = ScanRow {
            bt_id: None,
            name: "Beta".to_string(),
            rssi: 0,
        };
        assert_eq!(ScanColumns::Name.order_by(&row_a, &row_b), Ordering::Less);
        assert_eq!(ScanColumns::Name.order_by(&row_b, &row_a), Ordering::Greater);
    }

    #[test]
    fn order_by_name_equal_names() {
        let row_a = ScanRow {
            bt_id: None,
            name: "Same".to_string(),
            rssi: -70,
        };
        let row_b = ScanRow {
            bt_id: None,
            name: "Same".to_string(),
            rssi: -80,
        };
        assert_eq!(ScanColumns::Name.order_by(&row_a, &row_b), Ordering::Equal);
    }

    #[test]
    fn order_by_rssi_sorts_numerically() {
        let strong_signal = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: -50_i16,
        };
        let weak_signal = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: -90_i16,
        };
        // -90 < -50, so weak < strong
        assert_eq!(
            ScanColumns::Rssi.order_by(&weak_signal, &strong_signal),
            Ordering::Less
        );
        assert_eq!(
            ScanColumns::Rssi.order_by(&strong_signal, &weak_signal),
            Ordering::Greater
        );
    }

    #[test]
    fn order_by_rssi_equal_values() {
        let row_a = ScanRow {
            bt_id: None,
            name: "A".to_string(),
            rssi: -70_i16,
        };
        let row_b = ScanRow {
            bt_id: None,
            name: "B".to_string(),
            rssi: -70_i16,
        };
        assert_eq!(ScanColumns::Rssi.order_by(&row_a, &row_b), Ordering::Equal);
    }

    #[test]
    fn order_by_rssi_handles_min_rssi_sentinel() {
        // The code uses -200 as a sentinel for missing RSSI
        let missing_rssi = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: -200_i16,
        };
        let normal_rssi = ScanRow {
            bt_id: None,
            name: "".to_string(),
            rssi: -70_i16,
        };
        assert_eq!(
            ScanColumns::Rssi.order_by(&missing_rssi, &normal_rssi),
            Ordering::Less
        );
    }

    #[test]
    fn order_by_name_is_case_sensitive() {
        let lowercase = ScanRow {
            bt_id: None,
            name: "device".to_string(),
            rssi: 0,
        };
        let uppercase = ScanRow {
            bt_id: None,
            name: "Device".to_string(),
            rssi: 0,
        };
        // In standard string ordering, uppercase 'D' < lowercase 'd'
        assert_eq!(
            ScanColumns::Name.order_by(&uppercase, &lowercase),
            Ordering::Less
        );
    }

    // --- EnumIter tests ---

    #[test]
    fn scan_columns_iter_yields_both_variants() {
        use strum::IntoEnumIterator;
        let cols: Vec<ScanColumns> = ScanColumns::iter().collect();
        assert_eq!(cols.len(), 2);
        assert!(cols.contains(&ScanColumns::Name));
        assert!(cols.contains(&ScanColumns::Rssi));
    }

    #[test]
    fn scan_columns_iter_name_comes_first() {
        use strum::IntoEnumIterator;
        let cols: Vec<ScanColumns> = ScanColumns::iter().collect();
        assert_eq!(cols[0], ScanColumns::Name);
        assert_eq!(cols[1], ScanColumns::Rssi);
    }
}