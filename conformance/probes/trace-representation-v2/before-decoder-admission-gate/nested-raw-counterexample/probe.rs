use dive_value::{Ctx,Interner,Value,codec};
fn main(){for ctx in [Ctx::tlc(),Ctx::dive()] {
 let names=Interner::new();
 let a=Value::set_raw(vec![Value::Int(2),Value::Int(1)],ctx);
 let b=Value::set_raw(vec![Value::Int(3)],ctx);
 let value=codec::probe_raw_set(vec![a,b],ctx,true).unwrap();
 let j=dive_trace::value_repr::to_json(&value,ctx);
 println!("{ctx:?}: encoded {j:?}");
 if let Ok(j)=j { println!("decoded {:?}",dive_trace::value_repr::from_json(&j,&names,ctx)); }
}}
