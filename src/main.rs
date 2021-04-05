use serde_json::Value;
use std::fs::File;
use std::io::Write;
use std::io::Read;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::SystemTime;
use std::time::Duration;
struct Holder {
    jh: JoinHandle<()>,
    trans: Sender<Vec<i64>>,
}

#[derive(Debug, Clone)]
struct Extent {
    min: f64,
    max: f64,
}

// data, x and y
fn check_dist(
    data: &Value,
    i: i64,
    j: i64,
    x_extent:& Extent,
    y_extent:& Extent,
    size: i64,
) -> Vec<u8> {
    let mut minv = 1000.0;
    let mut minvec = vec![];
    let mut json_index: usize = 0;

    loop {
        if data["points"].get(json_index).is_none() {
            break;
        }
        let val = data["points"].get(json_index).unwrap();
        let x = val["x"].as_f64().unwrap();
        let y = val["y"].as_f64().unwrap();
        //println!("data point {} {}", x, y);
        let normx = (x - x_extent.min) / (x_extent.max - x_extent.min);
        let normy = (y - y_extent.min) / (y_extent.max - y_extent.min);
        //println!("normalized data point {} {}", normx, normy);
        let px = i as f64 / size as f64;
        let py = j as f64 / size as f64;
        //println!("screen point {} {}", px, py);
        let difx = (normx - px).abs();
        let dify = (normy - py).abs();
        //println!("dif {} {}", difx, dify);
        let dist = (difx.powf(2.0) + dify.powf(2.0)).powf(0.5);
        //println!("dist {}", dist);
        if dist < minv {
            minv = dist
        }
        //println!("point {:?}",data["points"].get(i));
        json_index += 1;
    }
    minvec.push(i as u8);
    minvec.push(j as u8);
    minvec.push((minv * 256.0) as u8);
    return minvec;
}

fn main() {
    let mut f = File::open("./test_vtu_convert_tstep10.json").unwrap();
    let mut contents = String::new();
    f.read_to_string(&mut contents).unwrap();
    // get the points
    //println!("contents {:?}",contents);
    let data: Value = serde_json::from_str(&contents).unwrap();
    let now = SystemTime::now();
    let size = 256*4;

    // determine the min and max for each value
    let mut json_index: usize = 0;
    let mut x_extent = Extent {
        min: 1000.0,
        max: -1000.0,
    };
    let mut y_extent = Extent {
        min: 1000.0,
        max: -1000.0,
    };
    loop {
        if data["points"].get(json_index).is_none() {
            break;
        }
        let val = data["points"].get(json_index).unwrap();
        let x = val["x"].as_f64().unwrap();
        let y = val["y"].as_f64().unwrap();
        if y < y_extent.min {
            y_extent.min = y;
        }
        if y > y_extent.max {
            y_extent.max = y;
        }
        if x < x_extent.min {
            x_extent.min = x;
        }
        if x > x_extent.max {
            x_extent.max = x;
        }
        //println!("point {:?}",data["points"].get(i));
        json_index += 1;
    }
    println!("extents {:?} {:?}", x_extent, y_extent);
    let mut my_threads = vec![];
    let (toriginal, roriginal) = mpsc::channel();
    let thread_num = 4;
    for t_num in 0..thread_num {
        // create n threads
        let local_data = data.clone();
        let local_x_extent = x_extent.clone();
        let local_y_extent = y_extent.clone();
        let toriginal_copy = toriginal.clone();
        // create the mpsc
        let (tlocal, rlocal) = mpsc::channel();
        let handle = thread::spawn(move || {
            // loop and compute, until we get a -1
            loop {
                // listen with the rlocal
                let res_vec: Vec<i64> = match rlocal.try_recv(){
                    Ok(v)=> v,
                    _=> {
                        thread::sleep(Duration::from_secs(1));
                        continue
                    }
                };
                if res_vec[0] == -1 {
                    break;
                }
                let x_inc = res_vec[0];
                let y_inc = res_vec[1];
                //
                let minv = check_dist(
                    &local_data,
                    x_inc,
                    y_inc,
                    &local_x_extent,
                    &local_y_extent,
                    size,
                );

                // send back result with the toriginal copy
                toriginal_copy.send(minv);
            }
        });
        my_threads.push(Holder {
            jh: handle,
            trans: tlocal,
        });
    }
    // now iterate over all the pixels in our size space and split up the values between the threads
    let mut thread_indexer = 0;
    for j in 0..size {
        for i in 0..size {
            thread_indexer += 1;
            my_threads[thread_indexer%thread_num].trans.send(vec![i,j]).expect("brken on send indices");
        }
    }
    // receive the valuees and store them up
    let mut all_results =vec![];
    let mut count = 0;
    for rec in roriginal {
        count += 1;
        all_results.push(rec);
        if count == size.pow(2) {
            break
        }
        //println!("{} out of {}",count,size.pow(2));
    }
    println!("time {:?}",now.elapsed());
    let mut f = File::create("output").unwrap();
    let str_res = format!("{:?}",all_results);
    f.write(str_res.as_bytes()).unwrap();

}
