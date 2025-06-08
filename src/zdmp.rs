use std::io::Cursor;
use std::io::Read;
use std::path::Path;

use std::io::Write;
use std::io::BufWriter;                                                                                                                                                  
use std::fs::File;  

use std::io::Seek;

use std::mem;

use std::time::{Instant};

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

pub const COMPRESSION_FORMAT_LZNT1:     u16 = 0x02;

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
            return Err(Error::DumpParseError);
        }

        info!("Zdmp file opened.");

        Ok(hdr)
    }
}

impl ZdmpBlockHdr {
    pub fn new(mut rdr: impl Read) -> Result<Self> {
        let hdr = read_type!(&mut rdr, ZdmpBlockHdr)?;

        if hdr.signature != ZDMP_BLOCK_SIGNATURE {
            return Err(Error::DumpParseError);
        }

        Ok(hdr)
    }
}

impl ZdmpFile {
    pub fn new(
        in_path: &Path,
        out_path: &Path,
        silent_mode: bool
    ) -> Result<Self> {
        info!("Parsing file...");

        let start_time = Instant::now(); 
        let mut file = File::open(in_path)?;

        // Reusable buffer for headers
        let mut hdr_buf = vec![0; mem::size_of::<ZdmpFileHdr>().max(mem::size_of::<ZdmpBlockHdr>())];

        file.seek(std::io::SeekFrom::Start(0))?;
        file.read_exact(&mut hdr_buf[..mem::size_of::<ZdmpFileHdr>()])?;
        let mut rdr = Cursor::new(&hdr_buf[..mem::size_of::<ZdmpFileHdr>()]);

        let zdmp_hdr = ZdmpFileHdr::new(&mut rdr)?;
        trace_multi!("zdmp_hdr", zdmp_hdr);

        let base = rdr.position();
        trace_func!("base: 0x{:x}", base);

        if zdmp_hdr.data_type != BLOCK_DATA_TYPE_COMPRESSION {
            return Err(Error::DumpParseError);
        }

        if zdmp_hdr.compression_format != COMPRESSION_FORMAT_LZNT1 {
            return Err(Error::DumpParseError);
        }

        let mut block_offset: u64 = ZDMP_BLOCK_START_OFFSET;
        let mut uncompressed_size = 0;
        let mut block_id = 0;

        let block_size = zdmp_hdr.block_size; 
        let file_size = file.metadata().unwrap().len();
        info!("hdr.block_size:      0x{:x}", block_size);
        info!("file_size:           0x{:x}", file_size);
        info!("zdmp_hdr.file_size:  0x{:x}", zdmp_hdr.file_size as usize);

        // Create buffered writer for better I/O performance
        let mut out_writer = if !silent_mode {
            Some(BufWriter::with_capacity(
                1024 * 1024, // 1MB buffer
                File::create(out_path)?
            ))
        } else {
            None
        };

        // Reusable buffers
        let mut uncompressed: Vec<u8> = Vec::with_capacity(block_size as usize);
        let mut block_data_buf: Vec<u8> = Vec::with_capacity(block_size as usize);

        while block_offset < file_size {
            if block_id % 1000 == 0 {
                info!("Processed {} blocks", block_id);
            }
            
            file.seek(std::io::SeekFrom::Start(block_offset))?;
            if file.read_exact(&mut hdr_buf[..mem::size_of::<ZdmpBlockHdr>()]).is_err() {
                info!("Error while reading block header #{} @ 0x{:x}. Is file corrupted?", block_id, block_offset);   
                break;
            }
            
            rdr = Cursor::new(&hdr_buf[..mem::size_of::<ZdmpBlockHdr>()]);
            let zdmp_block = ZdmpBlockHdr::new(&mut rdr)?;

            trace_multi!("zdmp_block", zdmp_block);

            if zdmp_block.data_size > zdmp_hdr.block_size {
                return Err(Error::DumpParseError);
            }
            
            let data_size = zdmp_block.data_size; 
            let crc32 = zdmp_block.crc32; 
            trace!("[{}] block.data_size:     0x{:x}", block_id, data_size);
            trace!("[{}] block.crc32:         0x{:x}", block_id, crc32);

            // Resize buffer only if needed
            if block_data_buf.len() < data_size as usize {
                block_data_buf.resize(data_size as usize, 0);
            }
            
            if file.read_exact(&mut block_data_buf[..data_size as usize]).is_err() {
                info!("Error while reading block @ 0x{:x}, 0x{:x} bytes", 
                    block_offset + mem::size_of::<ZdmpBlockHdr>() as u64, data_size);
                uncompressed_size += data_size as usize;
                
                if let Some(ref mut writer) = out_writer {
                    writer.write_all(&block_data_buf[..data_size as usize])?;
                }
            } else {
                let checksum = CRC32_IEEE.checksum(&block_data_buf[..data_size as usize]);
                trace!("[{}] crc32:               0x{:x}", block_id, checksum);

                if checksum != zdmp_block.crc32 {
                    return Err(Error::DumpParseError);
                }

                if zdmp_block.data_size != block_size {
                    uncompressed.clear();
                    if let Err(e) = lzxpress::lznt1::decompress2(&block_data_buf[..data_size as usize], &mut uncompressed) {
                        debug!("Decompression error: {:?}", e);
                    }

                    if uncompressed.len() > block_size as usize {
                        return Err(Error::DumpParseError);
                    }

                    // Pad if necessary
                    if uncompressed.len() < block_size as usize {
                        uncompressed.resize(block_size as usize, 0);
                    }

                    if let Some(ref mut writer) = out_writer {
                        writer.write_all(&uncompressed)?;
                    }
                    
                    uncompressed_size += uncompressed.len();
                } else {
                    // Not compressed
                    if let Some(ref mut writer) = out_writer {
                        writer.write_all(&block_data_buf[..data_size as usize])?;
                    }
                    uncompressed_size += data_size as usize;
                }
            }

            block_offset += mem::size_of::<ZdmpBlockHdr>() as u64;
            block_offset += zdmp_block.data_size as u64;
            block_id += 1;
        }

        // Flush the buffered writer
        if let Some(mut writer) = out_writer {
            writer.flush()?;
        }

        let finish_time = Instant::now();

        Ok(ZdmpFile { 
            hdr: zdmp_hdr, 
            file_size: zdmp_hdr.file_size, 
            block_count: block_id,
            uncompressed_size: uncompressed_size,
            start_time: start_time, 
            finish_time: finish_time
        })
    } 
}