use std::path::Path;
use image::GenericImageView;

struct Rec {
    index: u16,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bytes: Vec<u8>,
}

pub fn vec_equal(va: &[u8], vb: &[u8]) -> bool {
    va.len() == vb.len() && 
        va.iter().zip(vb).all(|(a,b)| *a==*b)
}
#[cfg(target_os = "windows")]
fn fix_path<P: AsRef<Path>>(p: P) -> String {
    const VERBATIM_PREFIX: &str = r#"\\?\"#;
    let p = p.as_ref().display().to_string();
    if p.starts_with(VERBATIM_PREFIX) {
        p[VERBATIM_PREFIX.len()..].to_string()
    } else {
        p
    }
}
fn main() {
    let mut dir = std::env::current_exe().unwrap();
    dir.pop();
    let dir = fix_path(dir);
    println!("{}",dir);
    let img1path = format!("{}/../../../examples/Screenshot1.png",dir);
    println!("{}",img1path);
    
    let img1 = image::open(&img1path).unwrap();
    let (width, height) = img1.dimensions();
    let bytes1 = img1.to_rgb8().into_raw();
    
    let img2 = image::open(&format!("{}/../../../examples/Screenshot2.png",dir)).unwrap();
    let bytes2 = img2.to_rgb8().into_raw();

    println!("img diff: {}", vec_equal(&bytes1, &bytes2));

    let rectangles1 = get_rectangles(&bytes1, width as u16, height as u16);
    let rectangles2 = get_rectangles(&bytes2, width as u16, height as u16);
    
    // sort 
    // rectangles1.sort_by_key(|r| r.index);
    // rectangles2.sort_by_key(|r| r.index);

    let mut different_indeces = Vec::new();
    
    for a in 0..rectangles1.len()-1 {
        let ra = &rectangles1[a];
        let rb = &rectangles2[a];
        if !vec_equal(&ra.bytes, &rb.bytes) {            
            different_indeces.push(a);            
        }
    }
    // for i in &different_indeces {
    //     println!("{}: different", i);
    // }
    println!("number of rects: {}", rectangles1.len());
    println!("different      : {}", different_indeces.len());
    println!("percent        : {}", different_indeces.len() as f32/rectangles1.len() as f32 * 100.0);


    // save rectangles
    // for i in &different_indeces {
    //     let r = &rectangles2[*i as usize];
    //     let name = format!("temp/Result_{}.png", r.index);
    //     image::save_buffer(&name, 
    //         &r.bytes, r.width as u32, r.height as u32, 
    //         image::ColorType::Rgb8).unwrap();   
    // }

}

fn get_rectangles(bytes: &[u8], swidth: u16, sheight: u16) -> Vec<Rec> {
    let side = 16 as u16;  
    let xrects = swidth as u16 / side;
    let rwidth = swidth as u16 % side;
    let yrects = sheight as u16 / side;
    let rheight = sheight as u16 % side;
    let pwidth = side * xrects; // partial width without reminder
    let pheight = side * yrects; // partial height without reminder
    let mut rectangles = Vec::<Rec>::new();
    
    let mut buffer: Vec<u8> = Vec::new();
    buffer.resize(side as usize * side as usize * 3, 0);

    let mut n = 0;
    for y in (0..pheight).step_by(side as usize) {
        for x in (0..pwidth).step_by(side as usize) {
            // println!("x={x},y={y}");
            let mut index = 0;
            for j in 0..side {
                let mut sindex = ((x as usize + ((y+j) as usize * swidth as usize)) * 3) as usize;
                for _ in 0..side {
                    buffer[index] = bytes[sindex];
                    buffer[index+1] = bytes[sindex+1];
                    buffer[index+2] = bytes[sindex+2];
                    index += 3;
                    sindex += 3;
                } 
            }
            n += 1;
            let rec = Rec {
                index: n,
                x: x as u16,
                y: y as u16,
                width: side as u16,
                height: side as u16,
                bytes : buffer.clone(),
            };
            rectangles.push(rec);

            //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
            //image::save_buffer(&name, &buffer, side, side, image::ColorType::Rgb8).unwrap();
        }
        n += 1;
    }

    // reminder column
    //println!("rwidth: {rwidth}, rheight: {rheight}");
    n = xrects+1;
    buffer.resize(rwidth as usize * side as usize * 3, 0);
    for y in (0..pheight).step_by(side as usize) {
        let mut index = 0;
        for j in 0..side {
            let mut sindex = ((pwidth as usize + ((y+j) as usize* swidth as usize)) * 3) as usize;
            for _ in 0..rwidth {
                //println!("sindex={sindex},y={y},index={index}");
                buffer[index] = bytes[sindex];
                buffer[index+1] = bytes[sindex+1];
                buffer[index+2] = bytes[sindex+2];
                index += 3;
                sindex += 3;
            } 
        }
        let rec = Rec {
            index: n,
            x: pwidth as u16,
            y: y as u16,
            width: rwidth as u16,
            height: side as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        n += xrects+1;
        //image::save_buffer(&name, &buffer, rwidth, side, image::ColorType::Rgb8).unwrap();
    }

    // reminder row
    n = (xrects+1) * yrects +1;
    buffer.resize(side as usize * rheight as usize * 3, 0);
    for x in (0..pwidth).step_by(side as usize) {
        let mut index = 0;
        for j in 0..rheight {
            let mut sindex = ((x as usize+ ((pheight+j) as usize * swidth as usize)) * 3) as usize;
            for _ in 0..side {
                buffer[index] = bytes[sindex];
                buffer[index+1] = bytes[sindex+1];
                buffer[index+2] = bytes[sindex+2];
                index += 3;
                sindex += 3;
            } 
        }
        let rec = Rec {
            index: n,
            x: x as u16,
            y: pheight as u16,
            width: side as u16,
            height: rheight as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        n += 1;
        //image::save_buffer(&name, &buffer, side, rheight, image::ColorType::Rgb8).unwrap();
    }

    // reminder last corner
    buffer.resize(rwidth as usize * rheight as usize * 3, 0);
    let mut index = 0;
    for j in 0..rheight {
        let mut sindex = ((pwidth as usize+ ((pheight+j) as usize* swidth as usize)) * 3) as usize;
        for _ in 0..rwidth {
            buffer[index] = bytes[sindex];
            buffer[index+1] = bytes[sindex+1];
            buffer[index+2] = bytes[sindex+2];
            index += 3;
            sindex += 3;
        } 
    }
    n = (xrects+1)*(yrects+1);
    let rec = Rec {
        index: n,
        x: pwidth as u16,
        y: pheight as u16,
        width: rwidth as u16,
        height: rheight as u16,
        bytes : buffer.clone(),
    };
    rectangles.push(rec);
    rectangles
}