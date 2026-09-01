use demo::trait_01_counter;
use demo::trait_02_generic;
use demo::trait_03_bounds;
use demo::trait_04_moba;
use demo::trait_05_moba_fn;
use demo::trait_06_moba_macro;

fn main() {
  trait_01_counter::demo();
  trait_02_generic::demo();
  trait_03_bounds::demo();
  trait_04_moba::demo();
  trait_05_moba_fn::demo();
  trait_06_moba_macro::demo();
}
