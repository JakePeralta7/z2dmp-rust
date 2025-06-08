use std::io::Cursor;
use std::io::Read;
use std::path::Path;

use std::io::Write;
use std::io::BufWriter;                                                                                                                                                  
use std::fs::File;  

use std::mem;

use std::time::{Instant};
use std::sync::Arc;
use std::thread;

use crate::result::{Result, Error};

use crc::{Crc, CRC_32_ISO_HDLC};
use std::str;
use lzxpress;
use memmap2::Mmap;
use crossbeam_channel::{bounded, Receiver, Sender};

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

#[derive(Debug, Clone)]
struct BlockInfo {
    id: u64,
    offset: u64,
    header: ZdmpBlockHdr,
}

struct ProcessedBlock {
    id: u64,
    data: Vec<u8>,
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
        let file = File::open(in_path)?;
        
        // Memory map the entire file for better performance
        let mmap = unsafe { Mmap::map(&file)? };
        let mmap = Arc::new(mmap);

        // Parse header
        let mut rdr = Cursor::new(&mmap[..mem::size_of::<ZdmpFileHdr>()]);
        let zdmp_hdr = ZdmpFileHdr::new(&mut rdr)?;
        trace_multi!("zdmp_hdr", zdmp_hdr);

        if zdmp_hdr.data_type != BLOCK_DATA_TYPE_COMPRESSION {
            return Err(Error::DumpParseError);
        }

        if zdmp_hdr.compression_format != COMPRESSION_FORMAT_LZNT1 {
            return Err(Error::DumpParseError);
        }

        let block_size = zdmp_hdr.block_size;
        let file_size = mmap.len() as u64;
        
        info!("hdr.block_size:      0x{:x}", block_size);
        info!("file_size:           0x{:x}", file_size);
        info!("zdmp_hdr.file_size:  0x{:x}", zdmp_hdr.file_size as usize);

        // Phase 1: Parse all block headers sequentially (fast)
        let mut block_infos = Vec::new();
        let mut block_offset = ZDMP_BLOCK_START_OFFSET;
        let mut block_id = 0;

        while block_offset < file_size {
            if block_offset + mem::size_of::<ZdmpBlockHdr>() as u64 > file_size {
                break;
            }

            let header_slice = &mmap[block_offset as usize..(block_offset + mem::size_of::<ZdmpBlockHdr>() as u64) as usize];
            let mut rdr = Cursor::new(header_slice);
            
            match ZdmpBlockHdr::new(&mut rdr) {
                Ok(zdmp_block) => {
                    if zdmp_block.data_size > zdmp_hdr.block_size {
                        info!("Invalid block size at offset 0x{:x}", block_offset);
                        break;
                    }
                    
                    block_infos.push(BlockInfo {
                        id: block_id,
                        offset: block_offset,
                        header: zdmp_block,
                    });
                    
                    block_offset += mem::size_of::<ZdmpBlockHdr>() as u64;
                    block_offset += zdmp_block.data_size as u64;
                    block_id += 1;
                }
                Err(_) => {
                    info!("Error parsing block header at offset 0x{:x}", block_offset);
                    break;
                }
            }
        }

        info!("Found {} blocks to process", block_infos.len());

        // Phase 2: Process blocks in parallel
        let num_threads = std::cmp::max(1, num_cpus::get());
        let chunk_size = std::cmp::max(1, block_infos.len() / (num_threads * 4)); // More chunks for better load balancing
        
        info!("Using {} threads with chunk size {}", num_threads, chunk_size);

        let (sender, receiver): (Sender<ProcessedBlock>, Receiver<ProcessedBlock>) = bounded(num_threads * 2);
        
        // Spawn worker threads
        let mut handles = Vec::new();
        for chunk in block_infos.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let mmap_clone = Arc::clone(&mmap);
            let sender_clone = sender.clone();
            let block_size = zdmp_hdr.block_size;
            
            let handle = thread::spawn(move || {
                // Pre-allocate buffers per thread
                let mut uncompressed_buf = Vec::with_capacity(block_size as usize * 2);
                let mut _temp_buf: Vec<u8> = Vec::with_capacity(block_size as usize);
                
                for block_info in chunk {
                    let data_start = block_info.offset + mem::size_of::<ZdmpBlockHdr>() as u64;
                    let data_end = data_start + block_info.header.data_size as u64;
                    
                    if data_end as usize <= mmap_clone.len() {
                        let block_data = &mmap_clone[data_start as usize..data_end as usize];
                        
                        // Verify CRC32
                        let checksum = CRC32_IEEE.checksum(block_data);
                        if checksum != block_info.header.crc32 {
                            info!("CRC mismatch for block {}", block_info.id);
                            continue;
                        }
                        
                        let processed_data = if block_info.header.data_size != block_size {
                            // Compressed block
                            uncompressed_buf.clear();
                            
                            match lzxpress::lznt1::decompress2(block_data, &mut uncompressed_buf) {
                                Ok(_) => {
                                    if uncompressed_buf.len() > block_size as usize {
                                        info!("Decompressed block {} too large", block_info.id);
                                        continue;
                                    }
                                    
                                    // Pad to block size
                                    uncompressed_buf.resize(block_size as usize, 0);
                                    uncompressed_buf.clone()
                                }
                                Err(e) => {
                                    info!("Decompression error for block {}: {:?}", block_info.id, e);
                                    continue;
                                }
                            }
                        } else {
                            // Uncompressed block
                            block_data.to_vec()
                        };
                        
                        let processed_block = ProcessedBlock {
                            id: block_info.id,
                            data: processed_data,
                        };
                        
                        if sender_clone.send(processed_block).is_err() {
                            break;
                        }
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Drop the original sender so receiver knows when all threads are done
        drop(sender);

        // Phase 3: Write blocks in order (if not silent mode)
        let mut uncompressed_size = 0;
        let mut next_expected_id = 0;
        let mut out_of_order_blocks: std::collections::HashMap<u64, ProcessedBlock> = std::collections::HashMap::new();
        
        let mut out_writer = if !silent_mode {
            Some(BufWriter::with_capacity(
                8 * 1024 * 1024, // 8MB buffer for better write performance
                File::create(out_path)?
            ))
        } else {
            None
        };

        while let Ok(processed_block) = receiver.recv() {
            if processed_block.id == next_expected_id {
                // Write this block and any subsequent ones we have
                if let Some(ref mut writer) = out_writer {
                    writer.write_all(&processed_block.data)?;
                }
                uncompressed_size += processed_block.data.len();
                next_expected_id += 1;
                
                // Check if we have subsequent blocks cached
                while let Some(cached_block) = out_of_order_blocks.remove(&next_expected_id) {
                    if let Some(ref mut writer) = out_writer {
                        writer.write_all(&cached_block.data)?;
                    }
                    uncompressed_size += cached_block.data.len();
                    next_expected_id += 1;
                }
            } else {
                // Cache out-of-order block
                out_of_order_blocks.insert(processed_block.id, processed_block);
            }
            
            if next_expected_id % 5000 == 0 {
                info!("Processed {} blocks", next_expected_id);
            }
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Flush the buffered writer
        if let Some(mut writer) = out_writer {
            writer.flush()?;
        }

        let finish_time = Instant::now();

        Ok(ZdmpFile { 
            hdr: zdmp_hdr, 
            file_size: zdmp_hdr.file_size, 
            block_count: block_infos.len() as u64,
            uncompressed_size: uncompressed_size,
            start_time: start_time, 
            finish_time: finish_time
        })
    } 
}