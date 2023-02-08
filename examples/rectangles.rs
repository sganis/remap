use image::GenericImageView;
use remap::util;

fn main() {
    let mut dir = std::env::current_exe().unwrap();
    dir.pop();
    let dir = util::fix_path(dir);
    println!("{}",dir);
    let img1path = format!("{}/../../../examples/Screenshot1.png",dir);
    println!("{}",img1path);
    
    let img1 = image::open(&img1path).unwrap();
    let (width, height) = img1.dimensions();
    let bytes1 = img1.to_rgba8().into_raw();
    
    let img2 = image::open(&format!("{}/../../../examples/Screenshot2.png",dir)).unwrap();
    let bytes2 = img2.to_rgba8().into_raw();

    println!("pixels: {}", width*height);
    println!("bytes : {}", bytes2.len());

    println!("img diff: {}", util::vec_equal(&bytes1, &bytes2));

    let rectangles1 = util::get_rectangles(&bytes1, width as u16, height as u16);
    let rectangles2 = util::get_rectangles(&bytes2, width as u16, height as u16);
    
    // sort 
    // rectangles1.sort_by_key(|r| r.index);
    // rectangles2.sort_by_key(|r| r.index);

    let mut different_indeces = Vec::new();
    
    for a in 0..rectangles1.len()-1 {
        let ra = &rectangles1[a];
        let rb = &rectangles2[a];
        if !util::vec_equal(&ra.bytes, &rb.bytes) {            
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

