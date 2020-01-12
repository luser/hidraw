#![allow(unused)]

#[derive(Debug)]
pub struct HidReportParserBuilder {
}

impl HidReportParserBuilder {
    pub fn new() -> HidReportParserBuilder {
        HidReportParserBuilder {
        }
    }

    pub fn build(self) -> HidReportParser {
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
enum Size {
    Bits(u8),
    Bytes(u8),
}

const AXIS_X: u8 = 0x30;
const AXIS_Y: u8 = 0x31;
const AXIS_Z: u8 = 0x32;
const AXIS_RZ: u8 = 0x35;

#[derive(Debug, Clone)]
enum What {
    Buttons {
        from: u8,
        to: u8,
    },
    Dpad {
        min: u8,
        max: u8,
    },
    Axis {
        usage: u8,
        min: u8,
        max: u8,
    },
    /// Constant items are used for padding out bytes.
    Const,
    Unknown,
}

#[derive(Debug, Clone)]
struct HidReportItem {
    /// The size of this item.
    size: Size,
    /// What this item represents.
    what: What,
}

#[derive(Debug, Clone)]
pub struct HidReportParser {
    inputs: Vec<HidReportItem>,
}

impl HidReportParser {
    pub fn len(&self) -> usize {
        let bits = self.inputs.iter().fold(0, |sum, i| sum + match i.size {
            Size::Bits(s) => s as usize,
            Size::Bytes(s) => (s as usize) * 8,
        });
        bits / 8
    }

    pub fn parse(&self, _report: &[u8]) {}
}

fn logitech_f310_parser() -> HidReportParser {
    HidReportParser {
        inputs: vec![
            HidReportItem { size: Size::Bytes(1), what: What::Axis { usage: AXIS_X, min: 0, max: 255 } },
            HidReportItem { size: Size::Bytes(1), what: What::Axis { usage: AXIS_Y, min: 0, max: 255 } },
            HidReportItem { size: Size::Bytes(1), what: What::Axis { usage: AXIS_Z, min: 0, max: 255 } },
            HidReportItem { size: Size::Bytes(1), what: What::Axis { usage: AXIS_RZ, min: 0, max: 255 } },
            HidReportItem { size: Size::Bits(4), what: What::Dpad { min: 0, max: 7 } },
            HidReportItem { size: Size::Bits(12), what: What::Buttons { from: 0x01, to: 0x0C } },
            HidReportItem { size: Size::Bytes(2), what: What::Unknown },
        ],
    }
}

pub fn find_report_parser_for_device(vendor_id: u16, product_id: u16) -> Option<HidReportParser> {
    if vendor_id == 0x046D && product_id == 0x0C216 {
        Some(logitech_f310_parser())
    } else {
        None
    }
}
