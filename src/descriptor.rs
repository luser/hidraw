use anyhow::{bail, Result};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read, Seek, SeekFrom};

const LONG_ITEM: u8 = 0b11111110;

const SIZE_MASK: u8 = 0b00000011;
const TYPE_MASK: u8 = 0b00001100;
const TAG_MASK: u8 = 0b11110000;

#[derive(Debug)]
enum ItemData {
    None,
    U8(u8),
    U16(u16),
    U32(u32),
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum ItemType {
    Main = 0,
    Global = 1,
    Local = 2,
    Reserved = 3,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum MainItemTag {
    Input = 0b1000,
    Output = 0b1001,
    Feature = 0b1011,
    Collection = 0b1010,
    EndCollection = 0b1100,
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum GlobalItemTag {
    UsagePage = 0b0000,
    LogicalMinimum = 0b0001,
    LogicalMaximum = 0b0010,
    PhysicalMinimum = 0b0011,
    PhysicalMaximum = 0b0100,
    UnitExponent = 0b0101,
    Unit = 0b0110,
    ReportSize = 0b0111,
    ReportID = 0b1000,
    ReportCount = 0b1001,
    Push = 0b1010,
    Pop = 0b1011,
    Reserved = 0b1100,
}

#[derive(Debug)]
enum ItemTag {
    Main(MainItemTag),
    Global(GlobalItemTag),
    Local(u8),
}

impl TryFrom<(u8, u8)> for ItemTag {
    type Error = anyhow::Error;
    fn try_from(value: (u8, u8)) -> Result<Self> {
        let ty = ItemType::try_from(value.0)?;
        match ty {
            ItemType::Global => Ok(ItemTag::Global(GlobalItemTag::try_from(value.1)?)),
            ItemType::Main => Ok(ItemTag::Main(MainItemTag::try_from(value.1)?)),
            ItemType::Local => Ok(ItemTag::Local(value.1)),
            ItemType::Reserved => bail!("Bad item type"),
        }
    }
}

pub fn parse_hid_descriptor(data: &[u8]) -> Result<()> {
    let mut cur = Cursor::new(data);
    let mut prefix = [0];
    while cur.read_exact(&mut prefix).is_ok() {
        let first = prefix[0];
        if first == LONG_ITEM {
            let mut long_desc = [0, 0];
            cur.read_exact(&mut long_desc)?;
            // Just skip over the data.
            let long_size = long_desc[0];
            cur.seek(SeekFrom::Current(long_size as i64))?;
        } else {
            let size = (first & SIZE_MASK) as usize;
            let ty = (first & TYPE_MASK) >> 2;
            let tag = (first & TAG_MASK) >> 4;
            let tag = ItemTag::try_from((ty, tag))?;
            let mut data_buf = [0, 0, 0, 0];
            if size > 0 {
                cur.read_exact(&mut data_buf[..size])?;
            }
            let data = match size {
                0 => ItemData::None,
                1 => ItemData::U8(data_buf[0]),
                2 => ItemData::U16(u16::from_le_bytes((&data_buf[..2]).try_into()?)),
                3 => ItemData::U32(u32::from_le_bytes(data_buf)),
                _ => unreachable!(),
            };
            println!("{tag:?}: {data:?}");
        }
    }
    Ok(())
}
