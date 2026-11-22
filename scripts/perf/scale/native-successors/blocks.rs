//! Bound operations used by source-emitted guard and write continuations.
use crate::handle::Handle;
use crate::node::Kind;
use crate::plan::{Cmp, Code, GNode, Node, P};
use crate::{Error, Machine};

#[derive(Clone, Copy, Debug)]
enum Atom {
    Constant(Handle),
    State(u16),
    Next(u16),
    Slot(u32),
}

impl Atom {
    fn bind(c: &Code, p: P) -> Option<Self> {
        Some(match *c.nodes.get(p as usize)? {
            Node::Const(h) => Self::Constant(h),
            Node::Var(v) => Self::State(v),
            Node::Primed(v) => Self::Next(v),
            Node::Slot(s) => Self::Slot(s),
            _ => return None,
        })
    }

    fn value(self, m: &Machine) -> Option<Handle> {
        let h = match self {
            Self::Constant(h) => h,
            Self::State(v) => *m.s0.get(usize::from(v))?,
            Self::Next(v) => *m.s1.get(usize::from(v))?,
            Self::Slot(s) => m.slot(s),
        };
        (!h.is_empty()).then_some(h)
    }
}

#[derive(Clone, Copy, Debug)]
enum Step { At(Atom), Field(Handle) }

#[derive(Clone, Debug)]
struct Read { root: Atom, path: Box<[Step]> }

impl Read {
    fn bind(c: &Code, mut p: P) -> Option<Self> {
        let mut path = Vec::new();
        while path.len() <= 4 {
            if let Some(root) = Atom::bind(c,p) {
                path.reverse();
                return Some(Self {root, path:path.into()});
            }
            match *c.nodes.get(p as usize)? {
                Node::Select(base,field) => { path.push(Step::Field(field)); p=base; }
                Node::Apply(base,key) => { path.push(Step::At(Atom::bind(c,key)?)); p=base; }
                _=>return None,
            }
        }
        None
    }

    fn get(&self, m: &Machine, r:&mut Register) -> Option<Handle> {
        let mut base=self.root.value(m)?;
        for (i,step) in self.path.iter().enumerate(){
            base=match *step {
                Step::At(a) => {let k=a.value(m)?; if i==0 {r.resolve(m,base,k)?.row} else {m.native_entry(base,k,false)?.1}},
                Step::Field(k) => m.native_entry(base,k,true)?.1,
            }
        }
        Some(base)
    }
}

#[derive(Clone,Copy,Debug)]
struct Cell { table:Handle, key:Handle, pos:usize, row:Handle }

/// A register lives on the stack for one emitted action. Neither a Store epoch
/// nor an arena rollback can extend its lifetime. Only immutable persistent
/// carriers may be retained across continuations.
#[derive(Default)]
pub(crate) struct Register(Option<Cell>);
impl Register {
    fn resolve(&mut self,m:&Machine,table:Handle,key:Handle)->Option<Cell>{
        if let Some(cell)=self.0 && cell.table==table && cell.key==key { return Some(cell); }
        let (pos,row)=m.native_entry(table,key,false)?;
        let cell=Cell{table,key,pos,row};
        if table.is_persistent() && !table.is_lazy_persistent() && !row.is_transient() { self.0=Some(cell); }
        Some(cell)
    }
}

#[derive(Clone,Debug)]
enum Predicate {
    Bool(Read), Same(Read,Read,bool), Compare(Cmp,Read,Read), Member(Read,Read,bool),
    Not(Box<Self>), All(Box<[Self]>), Any(Box<[Self]>),
}

impl Predicate {
    fn bind(c:&Code,p:P,depth:usize)->Option<Self>{
        if depth==0{return None}
        let bind=|p|Self::bind(c,p,depth-1);
        let read=|p|Read::bind(c,p);
        Some(match c.nodes.get(p as usize)? {
            Node::Memo{body,..}=>return bind(*body),
            Node::Eq(a,b)=>Self::Same(read(*a)?,read(*b)?,true),
            Node::Neq(a,b)=>Self::Same(read(*a)?,read(*b)?,false),
            Node::Cmp(cmp,a,b)=>Self::Compare(*cmp,read(*a)?,read(*b)?),
            Node::In(a,b)=>Self::Member(read(*a)?,read(*b)?,true),
            Node::NotIn(a,b)=>Self::Member(read(*a)?,read(*b)?,false),
            Node::Not(a)=>Self::Not(Box::new(bind(*a)?)),
            Node::And(v) if v.len()<=8=>Self::All(v.iter().map(|p|bind(*p)).collect::<Option<_>>()?),
            Node::Or(v) if v.len()<=8=>Self::Any(v.iter().map(|p|bind(*p)).collect::<Option<_>>()?),
            _=>Self::Bool(read(p)?),
        })
    }
    fn run(&self,m:&mut Machine,reg:&mut Register)->Option<bool>{
        match self {
            Self::Bool(a)=>a.get(m,reg)?.as_bool(),
            Self::Same(a,b,positive)=>{
                let a=a.get(m,reg)?;let b=b.get(m,reg)?;
                if (a.as_str_id().is_some()&&b.as_str_id().is_some()) || (a.as_int().is_some()&&b.as_int().is_some()) || (a.as_bool().is_some()&&b.as_bool().is_some()) || (a.as_model_id().is_some()&&a==b) {
                    Some((a==b)==*positive)
                } else {None}
            }
            Self::Compare(op,a,b)=>{
                let a=a.get(m,reg)?.as_int()?;let b=b.get(m,reg)?.as_int()?;
                Some(match op{Cmp::Lt=>a<b,Cmp::Le=>a<=b,Cmp::Gt=>a>b,Cmp::Ge=>a>=b})
            }
            Self::Member(a,b,positive)=>{
                let value=a.get(m,reg)?;let set=b.get(m,reg)?;
                if value.is_compound()|| !set.is_compound(){return None;}
                let v=m.heap.view(set);
                if v.kind!=Kind::Set||v.is_lazy()||v.data.len()>8{return None;}
                // Handle exactly closed homogeneous primitive operands.
                let yes= if v.data.is_empty() { false }
                    else if value.as_str_id().is_some() && v.data.iter().all(|h|h.as_str_id().is_some())
                    ||value.as_int().is_some()&&v.data.iter().all(|h|h.as_int().is_some())
                    ||value.as_bool().is_some()&&v.data.iter().all(|h|h.as_bool().is_some()) {
                        v.data.contains(&value)
                    }else{return None};
                Some(yes==*positive)
            }
            Self::Not(a)=>Some(!a.run(m,reg)?),
            Self::All(xs)=>{for x in xs {if !x.run(m,reg)?{return Some(false)}}Some(true)},
            Self::Any(xs)=>{for x in xs {if x.run(m,reg)?{return Some(true)}}Some(false)}
        }
    }
}

#[derive(Clone,Debug)]
struct Write {rhs:P,table:Atom,actor:Atom}

/// Exact operands derived once from the current compiled value nodes.
#[derive(Clone,Debug,Default)]
pub struct Blocks { guards:Vec<Option<Predicate>>, writes:Vec<Option<Write>> }
impl Blocks {
    pub(crate) fn bind(c:&Code)->Self{
        let mut guards=Vec::with_capacity(c.gnodes.len()); let mut writes=Vec::with_capacity(c.gnodes.len());
        for node in &c.gnodes{
            let pred=match node {
                GNode::Guard(p)|GNode::LabelGuard(p,_)=>Some(*p),
                GNode::If{cond,..}=>Some(*cond),GNode::Implies{lhs,..}=>Some(*lhs),_=>None
            }.and_then(|p|Predicate::bind(c,p,5));
            guards.push(pred);
            let w=match node {
                GNode::Assign{rhs,..}=>{let rhs=*rhs;
                    if let Node::Except{base,updates,row_fields:Some(_)}=&c.nodes[rhs as usize] {
                        Atom::bind(c,*base).zip(updates.first().and_then(|u|u.path.first()).and_then(|p|Atom::bind(c,*p)))
                            .map(|(table,actor)|Write{rhs,table,actor})
                    }else{None}
                },_=>None};
            writes.push(w);
        }
        Self{guards,writes}
    }

    pub(crate) fn test(&self,m:&mut Machine,r:&mut Register,g:usize)->Option<bool>{
        self.guards.get(g)?.as_ref()?.run(m,r)
    }

    pub(crate) fn write(&self,m:&mut Machine,c:&Code,r:&mut Register,g:usize)->Result<Option<Handle>,Error>{
        let Some(w)=self.writes.get(g).and_then(Option::as_ref) else{return Ok(None)};
        let Some((table,key))=w.table.value(m).zip(w.actor.value(m))else{return Ok(None)};
        let Some(cell)=r.resolve(m,table,key)else{return Ok(None)};
        let Node::Except{updates,row_fields:Some(plan),..}=&c.nodes[w.rhs as usize]else{return Ok(None)};
        m.native_row_write(c,table,updates,plan,cell.pos,cell.row)
    }
}
