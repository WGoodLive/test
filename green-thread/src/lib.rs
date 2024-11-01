#[cfg(test)]

#[test]
fn vec_append(){
    let mut a = vec![2,3];
    let mut b = vec![3,4];
    a.append(&mut b);
    println!("{:#?}",a);
}