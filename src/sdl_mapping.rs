use uuid::{Bytes, Uuid};


pub fn create_sdl_controller_uuid(bus: u16, vendor: u16, product: u16, version: u16) -> Uuid {
    // These parameters come from `struct input_id` via the `EVIOCGID` ioctl:
    // https://github.com/torvalds/linux/blob/9d646009f65d62d32815f376465a3b92d8d9b046/include/uapi/linux/input.h#L59
    // SDL has additional parameters that we're omitting here:
    // - The name is used to calculate a CRC, but it is not used for finding mappings, and Linux mappings don't include a crc field
    // - driver_signature and driver_data, which are always 0 on Linux
    let mut bytes: Bytes = Default::default();
    let parts = &[bus, 0, vendor, 0, product, 0, version, 0];
    for (chunk, part) in bytes.chunks_exact_mut(2).zip(parts.iter()) {
        chunk.copy_from_slice(&part.to_le_bytes());
    }
    Uuid::from_bytes(bytes)
}

// 050000007e0500003003000001000000,Nintendo Wii U Pro Controller,a:b0,b:b1,back:b8,dpdown:b14,dpleft:b15,dpright:b16,dpup:b13,guide:b10,leftshoulder:b4,leftstick:b11,lefttrigger:b6,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:b7,rightx:a2,righty:a3,start:b9,x:b3,y:b2,platform:Linux,
