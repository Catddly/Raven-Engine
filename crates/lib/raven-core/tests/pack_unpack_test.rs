use std::collections::HashMap;
use std::path::PathBuf;

use parking_lot::Mutex;

use raven_core::asset::Test;

lazy_static::lazy_static! {
    static ref ASSET_MMAPS: Mutex<HashMap<PathBuf, memmap2::Mmap>> = Mutex::new(HashMap::new());
}

const PACKED_FILE_NAME: &'static str = "test.packed";

fn write_packed_byte_buffer() {
    let test = Test::Asset {
        field_0: 8,
        field_1: 23,
        field_2: 535,
        field_3: vec![9, 7, 6, 5, 456],
    };

    let mut file = std::fs::File::create(PACKED_FILE_NAME).unwrap();
    test.write_packed(&mut file);
}

#[test]
fn test_pack_unpacked() {
    use memmap2;

    write_packed_byte_buffer();

    // read back using memory mapped buffer
    let mut asset_map = ASSET_MMAPS.lock();
    let field_reader;
    {
        let data: &[u8] = {
            asset_map.entry(PathBuf::from(PACKED_FILE_NAME)).or_insert_with(|| {
                let file = std::fs::File::open(PACKED_FILE_NAME).unwrap();
    
                unsafe { memmap2::MmapOptions::new().map(&file).unwrap() }
            })
        };
    
        field_reader = Test::get_field_reader(data.as_ptr() as *const Test::Packed);
    }
    let vec = field_reader.field_3().to_vec();
    
    println!("{:?}", field_reader.field_0());
    println!("{:?}", field_reader.field_1());
    println!("{:?}", field_reader.field_2());
    println!("{:?}", vec);

    assert_eq!(field_reader.field_0(), 8);
    assert_eq!(field_reader.field_1(), 23);
    assert_eq!(field_reader.field_2(), 535);

    for (idx, elem) in [9, 7, 6, 5, 456].into_iter().enumerate() {
        assert_eq!(vec[idx], elem);
    }
}