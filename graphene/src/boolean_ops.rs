use crate::{
   layers::{
      simple_shape::Shape,
   },
   intersection,
};

pub enum BooleanOperation{
   Union,
   Difference,
   Intersection,
   SubFront,
   SubBack,
}

pub fn boolean_operation(type: BooleanOperation, shapes: Vec<Shape>){

}
