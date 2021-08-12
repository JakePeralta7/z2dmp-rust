fn bytes_to_chars(bytes: &Vec<u8>) -> String
{
    let mut s = String::new();

    for byte in bytes {
        if *byte >= 32 && *byte <= 126 {
            s += &*format!("{}", *byte as char);
        } else {
            s += &*format!(".");
        }
    }

    s
}

pub fn hexdump(addr: u64, bytes: &Vec<u8>) -> String
{
    let mut s = String::new();
    let mut line = Vec::new();

    let columns = 16;

    for (i, byte) in bytes.iter().enumerate() {
        // Prepend the address.
        if i % columns == 0 {
            s += &*format!("{:016x}: ", addr + i as u64);
        }

        // Do not add newline for the very last character.
        if (i + 1) % columns == 0 && (i + 1) == bytes.len() {
            s += &*format!("{:02x}  {}", byte, bytes_to_chars(&line));
            line = Vec::new();

        // End of the line.
        }  else if (i + 1) % columns == 0 {
            s += &*format!("{:02x}  {}\n", byte, bytes_to_chars(&line));
            line = Vec::new();

        // Do not add newline for the very last character.
        } else if (i + 1) == bytes.len() {
            s += &*format!("{:02x}", byte);
            line.push(*byte);

        // Otherwise.
        } else {
            s += &*format!("{:02x} ", byte);
            line.push(*byte);
        }
    }

    // Process leftovers.
    if !line.is_empty() && line.len() != columns {
        // Insert padding for missing characters.
        for _ in 0..(columns - line.len()) {
            s += &*format!("{} ", "  ")
        }

        s += &*format!("  {}", bytes_to_chars(&line));
    }

    s
}