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
fn main() {
    let img1 = image::open("C:\\Users\\san\\Pictures\\Screenshots\\Screenshot1.png").unwrap();
    let img2 = image::open("C:\\Users\\san\\Pictures\\Screenshots\\Screenshot2.png").unwrap();
    
    let bytes1 = img1.to_rgb8().into_raw();
    let bytes2 = img2.to_rgb8().into_raw();
    let (swidth, sheight) = img1.dimensions();
    let swidth = swidth as u16;
    let sheight = sheight as u16;

    println!("dimensions 1 {:?}", img1.dimensions());
    println!("dimensions 2 {:?}", img2.dimensions());
    println!("bytes len: {}", bytes1.len());
    
    println!("img diff: {}", vec_equal(&bytes1, &bytes2));
    
    let mut rectangles = Vec::<Rec>::new();

    let lwidth = 200 as u16;
    let lheight = 200 as u16;
    
    let mut buffer: Vec<u8> = Vec::new();
    buffer.resize(lwidth as usize * lheight as usize * 3, 0);

    let xrects = swidth as u16 / lwidth;
    let rwidth = swidth as u16 % lwidth;
    let yrects = sheight as u16 / lheight;
    let rheight = sheight as u16 % lheight;
    let pwidth = lwidth * xrects; // partial width without reminder
    let pheight = lheight * yrects; // partial height without reminder
    


    let mut n = 0;
    for y in (0..pheight).step_by(lheight as usize) {
        for x in (0..pwidth).step_by(lwidth as usize) {
            println!("x={x},y={y}");
            let mut index = 0;
            for j in 0..lheight {
                let mut sindex = ((x as usize + ((y+j) as usize * swidth as usize)) * 3) as usize;
                for _ in 0..lwidth {
                    buffer[index] = bytes1[sindex];
                    buffer[index+1] = bytes1[sindex+1];
                    buffer[index+2] = bytes1[sindex+2];
                    index += 3;
                    sindex += 3;
                } 
            }
            n += 1;
            let rec = Rec {
                index: n,
                x: x as u16,
                y: y as u16,
                width: lwidth as u16,
                height: lheight as u16,
                bytes : buffer.clone(),
            };
            rectangles.push(rec);

            //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
            //image::save_buffer(&name, &buffer, lwidth, lheight, image::ColorType::Rgb8).unwrap();
        }
        n += 1;
    }

    // reminder column
    println!("rwidth: {rwidth}, rheight: {rheight}");
    n = xrects+1;
    buffer.resize(rwidth as usize * lheight as usize * 3, 0);
    for y in (0..pheight).step_by(lheight as usize) {
        let mut index = 0;
        for j in 0..lheight {
            let mut sindex = ((pwidth as usize + ((y+j) as usize* swidth as usize)) * 3) as usize;
            for _ in 0..rwidth {
                //println!("sindex={sindex},y={y},index={index}");
                buffer[index] = bytes1[sindex];
                buffer[index+1] = bytes1[sindex+1];
                buffer[index+2] = bytes1[sindex+2];
                index += 3;
                sindex += 3;
            } 
        }
        let rec = Rec {
            index: n,
            x: pwidth as u16,
            y: y as u16,
            width: rwidth as u16,
            height: lheight as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        n += xrects+1;
        //image::save_buffer(&name, &buffer, rwidth, lheight, image::ColorType::Rgb8).unwrap();
    }

    // reminder row
    n = (xrects+1) * yrects +1;
    buffer.resize(lwidth as usize * rheight as usize * 3, 0);
    for x in (0..pwidth).step_by(lwidth as usize) {
        let mut index = 0;
        for j in 0..rheight {
            let mut sindex = ((x as usize+ ((pheight+j) as usize * swidth as usize)) * 3) as usize;
            for _ in 0..lwidth {
                buffer[index] = bytes1[sindex];
                buffer[index+1] = bytes1[sindex+1];
                buffer[index+2] = bytes1[sindex+2];
                index += 3;
                sindex += 3;
            } 
        }
        let rec = Rec {
            index: n,
            x: x as u16,
            y: pheight as u16,
            width: lwidth as u16,
            height: rheight as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        n += 1;
        //image::save_buffer(&name, &buffer, lwidth, rheight, image::ColorType::Rgb8).unwrap();
    }

    // reminder last corner
    buffer.resize(rwidth as usize * rheight as usize * 3, 0);
    let mut index = 0;
    for j in 0..rheight {
        let mut sindex = ((pwidth as usize+ ((pheight+j) as usize* swidth as usize)) * 3) as usize;
        for _ in 0..rwidth {
            buffer[index] = bytes1[sindex];
            buffer[index+1] = bytes1[sindex+1];
            buffer[index+2] = bytes1[sindex+2];
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

    // save rectangles
    for r in rectangles.iter() {
        let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{}.png",r.index);
        image::save_buffer(&name, &r.bytes, r.width as u32, r.height as u32, image::ColorType::Rgb8).unwrap();   
    }

}