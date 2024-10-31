use std::sync::Arc;

struct A(u8);

fn main() {
    let a = Arc::new(A(6));
    r(a);
    // println!("{}",a  .0)
}
fn r(y:Arc<A>){
    println!("{}",y.0)
}
