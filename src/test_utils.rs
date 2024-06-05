pub fn all_near(a: &Vec<f32>, b: &Vec<f32>, epsilon: f32) {
    if a.len() != b.len() {
        println!("");
        println!("left = {:?}\nright = {:?}", a, b);
        println!("");
        panic!("lengths differ: {} != {}", a.len(), b.len());
    }
    let near = a
        .iter()
        .zip(b.iter())
        .map(|(a, b)| (a - b).abs())
        .all(|x| x < epsilon);
    println!("");
    assert!(near, "left = {:?}\nright = {:?}", a, b);
    println!("");
}
