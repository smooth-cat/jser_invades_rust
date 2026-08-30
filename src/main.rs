use demo::counter;
use demo::moba_direct;
use demo::moba_direct_macro;
use demo::moba_composite;
use demo::moba_generic;

fn main() {
  counter::demo();
  println!("----------------------- Hero -----------------------");
  moba_direct::demo();
  moba_direct_macro::demo();
  moba_composite::demo();
  moba_generic::demo();
}
