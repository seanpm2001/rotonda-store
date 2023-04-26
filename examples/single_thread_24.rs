use log::trace;
use std::time::Duration;
use std::thread;

use rand::Rng;

use rotonda_store::prelude::*;
use rotonda_store::prelude::multi::*;

use rotonda_store::meta_examples::PrefixAs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "cli")]
    env_logger::init();

    trace!("Starting multi-threaded yolo testing....");
    let tree_bitmap = MultiThreadedStore::<PrefixAs>::new().unwrap();

    let mut pfx_int = 0_u32;

    let thread = std::thread::Builder::new()
        .name(1_u8.to_string())
        .spawn(move || -> Result<(), Box<dyn std::error::Error + Send>> {
            let mut rng= rand::thread_rng();

            println!("park thread {}", 1);
            // thread::park();

            print!("\nstart {} ---", 1);

            while pfx_int <= 24 {
                pfx_int += 1;
                let pfx = Prefix::new_relaxed(
                    pfx_int.into_ipaddr(),
                    32,
                );
                
                let asn: u32 = rng.gen();
                match tree_bitmap.insert(&pfx.unwrap(), PrefixAs(asn)) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("{}", e);
                    }
                };
            }

            drop(tree_bitmap);
            Ok(())

        })
        .unwrap();
    
    // thread.thread().unpark();

    // thread::sleep(Duration::from_secs(10));

    thread.join().unwrap().unwrap();

    println!("------ end of inserts\n");

    // let guard = unsafe { epoch::unprotected() };

    // let s_spfx = tree_bitmap.match_prefix(
    //     &pfx.unwrap(),
    //     &MatchOptions {
    //         match_type: rotonda_store::MatchType::ExactMatch,
    //         include_all_records: true,
    //         include_less_specifics: true,
    //         include_more_specifics: true,
    //     },
    //     guard,
    // );
    // println!("query result");
    // println!("{}", s_spfx);
    // println!("{}", s_spfx.more_specifics.unwrap());
    
    println!("-----------");

    Ok(())
}
