use std::io::Cursor;
use std::io::Read;
use std::path::Path;

use std::io::Write;                                                                                                                                                                  
// use std::io::prelude::*;                                                                                                                                                             
use std::fs::File;  

use std::io::Seek;

use std::mem;

use std::time::{Instant, Duration};

use crate::result::{Result, Error};

use crc::{Crc, CRC_32_ISO_HDLC};
use std::str;
use lzxpress;

pub const CRC32_IEEE: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub const ZDMP_FILE_SIGNATURE:      u32 = 0x504d_445a;  // ZDMP
pub const ZDMP_BLOCK_SIGNATURE:     u32 = 0x4b4c_425a;  // KLBZ

pub const ZDMP_FILE_VERSION_10:     u32 = 0x0100;
pub const PAGE_SIZE:                usize = 0x1000;
pub const ZDMP_BLOCK_START_OFFSET:  u64  = 0x1000;

pub const BLOCK_DATA_TYPE_NONE:         u16 = 0x00;
pub const BLOCK_DATA_TYPE_COMPRESSION:  u16 = 0x01;
pub const BLOCK_DATA_TYPE_ENCRYPTION:   u16 = 0x02;

pub const COMPRESSION_FORMAT_LZNT1;     u16 = 0x02;

/// ZDMP File Header
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ZdmpFileHdr {
    pub signature:          u32,
    pub version:            u32,
    pub file_size:          u64,
    pub block_size:         u32,
    pub data_type:          u16,
    pub compression_format: u16
}

/// ZDMP Block Header
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ZdmpBlockHdr {
    pub signature:          u32,
    pub data_size:          u32,
    pub crc32:              u32,
}

/// ZDMP Block Header
#[derive(Debug)]
pub struct ZdmpFile {
    pub hdr:                ZdmpFileHdr,
    pub block_count:        u64,
    pub file_size:          u64,
    pub uncompressed_size:  usize,
    pub start_time:         Instant,
    pub finish_time:        Instant
}

impl ZdmpFileHdr {
    pub fn new(mut rdr: impl Read) -> Result<Self> {
        let hdr = read_type!(&mut rdr, ZdmpFileHdr)?;

        if hdr.signature != ZDMP_FILE_SIGNATURE {
            return Err(Error::DumpParseError(
                format!("Unexpected zdump signature field: 0x{:x}",
                    { hdr.signature })));
        }

        info!("Zdmp file opened.");

        Ok(hdr)
    }
}


impl ZdmpBlockHdr {
    pub fn new(mut rdr: impl Read) -> Result<Self> {
        let hdr = read_type!(&mut rdr, ZdmpBlockHdr)?;

        if hdr.signature != ZDMP_BLOCK_SIGNATURE {
            return Err(Error::DumpParseError(
                format!("Unexpected zdump block signature field: 0x{:x}",
                    { hdr.signature })));
        }

        Ok(hdr)
    }
}

impl ZdmpFile {
    pub fn new(in_path: &Path, out_path: &Path) -> Result<Self> {
        info!("Parsing file...");

        let start_time = Instant::now(); 
        let mut file = File::open(in_path)?;

        let mut buf = vec![0; mem::size_of::<ZdmpFileHdr>()];

        file.seek(std::io::SeekFrom::Start(0))?;
        file.read_exact(&mut buf)?;
        let mut rdr = Cursor::new(buf);

        let zdmp_hdr = ZdmpFileHdr::new(&mut rdr)?;
        trace_multi!("zdmp_hdr", zdmp_hdr);

        let base = rdr.position();
        trace_func!("base: 0x{:x}", base);

        if zdmp_hdr.data_type != BLOCK_DATA_TYPE_COMPRESSION {
            return Err(Error::DumpParseError(
                format!("Unsupported dump file.")));  
        }

        if zdmp_hdr.compression_format != COMPRESSION_FORMAT_LZNT1 {
            return Err(Error::DumpParseError(
                format!("Unsupported compression algorithm.")));  
        }

        let mut block_offset: u64 = ZDMP_BLOCK_START_OFFSET;
        let mut uncompressed_size = 0;
        let mut block_id = 0;

        let block_size = zdmp_hdr.block_size; 
        info!("hdr.block_size:      0x{:x}", block_size);

        let mut out_file = File::create(out_path).expect("Err: Unable to create file"); 
        
        while block_offset < zdmp_hdr.file_size {
            info!("Block @ 0x{:x}", block_offset);
            let mut block_hdr_buf = vec![0; mem::size_of::<ZdmpBlockHdr>()];
            file.seek(std::io::SeekFrom::Start(block_offset))?;
            file.read_exact(&mut block_hdr_buf)?;
            rdr = Cursor::new(block_hdr_buf);
            let zdmp_block = ZdmpBlockHdr::new(&mut rdr)?;

            trace_multi!("zdmp_block", zdmp_block);

            if zdmp_block.data_size > zdmp_hdr.block_size {
                return Err(Error::DumpParseError(
                    format!("Unexpected zdump block size: 0x{:x}",
                        { zdmp_block.data_size })));
            }
            
            let data_size = zdmp_block.data_size; 
            let crc32 = zdmp_block.crc32; 
            info!("[{}] block.data_size:     0x{:x}", block_id, data_size);
            info!("[{}] block.crc32:         0x{:x}", block_id, crc32);

            let mut block_data_buf = vec![0; zdmp_block.data_size as usize];
            file.read_exact(&mut block_data_buf)?;
            // info!("{:02X?}", block_data_buf);
            // rdr = Cursor::new(block_data_buf);
            let checksum = CRC32_IEEE.checksum(&block_data_buf);
            info!("[{}] crc32:               0x{:x}", block_id, checksum);

            if checksum != zdmp_block.crc32 {
                return Err(Error::DumpParseError(
                    format!("Incorrect crc32. 0x{:x} (expected 0x{:x})",
                        checksum,  crc32)));  
            }

            if zdmp_block.data_size != block_size {
                let uncompressed = lzxpress::lznt1::decompress(&block_data_buf).unwrap();
                info!("[{}] uncompressed.len():  0x{:x}", block_id, uncompressed.len());

                if uncompressed.len() != block_size as usize {
                    return Err(Error::DumpParseError(
                        format!("Incorrect uncompressed block size. 0x{:x} (expected 0x{:x})",
                            uncompressed.len(),  block_size)));  
                }

                let data_bytes: &[u8] = &uncompressed;       
                out_file.write_all(data_bytes).expect("Unable to write data");
                
                uncompressed_size += uncompressed.len();
            } else {
                uncompressed_size += block_data_buf.len();
                // Not compressed.
                let data_bytes: &[u8] = &block_data_buf;
                out_file.write_all(data_bytes).expect("Unable to write data");
            }

            block_offset += mem::size_of::<ZdmpBlockHdr>() as u64;
            block_offset += zdmp_block.data_size as u64;
            block_id += 1;
        }

        let finish_time = Instant::now();

        Ok(ZdmpFile { hdr: zdmp_hdr, file_size: zdmp_hdr.file_size, 
            block_count: block_id,
            uncompressed_size: uncompressed_size,
            start_time: start_time, finish_time: finish_time})
    } 
}