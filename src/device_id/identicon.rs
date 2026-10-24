//! The little picture Syncthing's web UI draws for a device ID, on the console.

use super::DeviceId;

/// The web UI's identicon of `device_id` (gui/default/syncthing/core/identiconDirective.js):
/// a 5x5 grid, mirrored left to right; the cell at (row, column) of the left half is filled
/// when the character at `row + column * 5` of the ID's letters and digits has an even code.
pub fn grid(device_id: &str) -> [[bool; 5]; 5] {
    let chars: Vec<u8> = device_id
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .collect();
    let mut grid = [[false; 5]; 5];
    for (row, cells) in grid.iter_mut().enumerate() {
        for col in 0..3 {
            // A missing character is NaN in the web UI's JavaScript, which counts as even.
            if chars.get(row + col * 5).is_none_or(|c| c % 2 == 0) {
                cells[col] = true;
                cells[4 - col] = true;
            }
        }
    }
    grid
}

/// The identicon's lines: two block characters per cell, which comes out about square.
pub fn rows(device_id: &DeviceId) -> Vec<String> {
    grid(device_id.as_str())
        .iter()
        .map(|cells| {
            cells
                .iter()
                .map(|&filled| if filled { "██" } else { "  " })
                .collect()
        })
        .collect()
}

/// The identicon as an indented block of text.
pub fn render(device_id: &DeviceId) -> String {
    rows(device_id)
        .iter()
        .map(|row| format!("    {row}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read off Syncthing v2.1.7's web UI for four test devices ('#' = filled).
    #[test]
    fn matches_the_web_ui() {
        let cases = [
            (
                "XMSTKFQ-TPD43NY-3Y4WQP3-NT5WLT3-IPV6T6M-4LEZVZD-WIDV2DC-RUCDPAO",
                "#####|     | ### |## ##| # # ",
            ),
            (
                "NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP",
                "#   #|# # #|#   #|#   #|  #  ",
            ),
            (
                "RFOHASD-DRBXSO6-QX7U35B-YIBWXRR-KF2R7GP-OEV6RK7-ADTRQJG-6LCMCQQ",
                "# # #|## ##| # # |#####| # # ",
            ),
            (
                "2DC3CEY-22LTKYB-L2DVONG-WNAUPTF-BA7TLCA-EDB63AI-Q2OGU3X-GHYTTAB",
                "# # #|#   #| # # | ### | ### ",
            ),
        ];
        for (id, expected) in cases {
            let drawn: Vec<String> = grid(id)
                .iter()
                .map(|cells| cells.iter().map(|&c| if c { '#' } else { ' ' }).collect())
                .collect();
            assert_eq!(drawn.join("|"), expected, "{id}");
        }
    }

    #[test]
    fn mirrors_left_to_right() {
        for cells in grid("NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP") {
            assert_eq!(cells[0], cells[4]);
            assert_eq!(cells[1], cells[3]);
        }
    }

    #[test]
    fn fills_cells_of_even_characters() {
        // Index row + col * 5: the first column is the first five characters.
        // 'B' (66) and 'D' (68) are even, 'A' (65), 'C' (67) and 'E' (69) odd.
        let grid = grid("ABCDE-AAAAA-AAAAA");
        let first_column: Vec<bool> = grid.iter().map(|cells| cells[0]).collect();
        assert_eq!(first_column, [false, true, false, true, false]);
        assert!(grid.iter().all(|cells| !cells[1] && !cells[2]));
    }
}
