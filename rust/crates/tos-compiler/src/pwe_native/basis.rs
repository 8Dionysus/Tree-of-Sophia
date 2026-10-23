use serde_json::Value;
use std::{mem::size_of, sync::atomic::{AtomicBool, Ordering}, time::Instant};
use tos_foundation::Digest256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(&'static str), NeedsBinding(&'static str), Unsupported(&'static str),
    ExactMismatch, QualificationMismatch(&'static str), ResourceLimit, Cancelled, Deadline,
    Denied, Upstream(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Incremental PWE charges. Upstream parser/source/schema worker holders remain
/// caller-owned. Zero limits refuse; there is no process-global default budget.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize, pub max_nodes: usize, pub max_work: usize,
    pub max_refs: usize, pub max_depth: usize, pub max_items: usize,
    pub max_state_bytes: usize, pub deadline: Instant,
}
pub struct Session<'a> {
    pub(super) limits: Limits, cancelled: &'a AtomicBool,
    bytes: usize, nodes: usize, work: usize, refs: usize, state: usize,
}
fn add(n: &mut usize, d: usize, cap: usize) -> Result<()> {
    let next = n.checked_add(d).ok_or(Error::ResourceLimit)?;
    if next > cap { return Err(Error::ResourceLimit); }
    *n = next; Ok(())
}
impl<'a> Session<'a> {
    pub fn new(limits: Limits, cancelled: &'a AtomicBool) -> Result<Self> {
        if [limits.max_input_bytes, limits.max_nodes, limits.max_work, limits.max_refs,
            limits.max_items, limits.max_state_bytes, limits.max_depth].contains(&0)
            || limits.max_depth > 96 { return Err(Error::ResourceLimit); }
        let s = Self { limits, cancelled, bytes:0, nodes:0, work:0, refs:0, state:0 };
        s.check()?; Ok(s)
    }
    pub fn charges(&self) -> (usize, usize, usize, usize, usize) {
        (self.bytes, self.nodes, self.work, self.refs, self.state)
    }
    pub(super) fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) { return Err(Error::Cancelled); }
        if Instant::now() >= self.limits.deadline { return Err(Error::Deadline); } Ok(())
    }
    pub(super) fn tick(&mut self, n: usize) -> Result<()> {
        self.check()?; add(&mut self.work, n, self.limits.max_work)
    }
    pub(super) fn state(&mut self, n: usize) -> Result<()> {
        self.check()?; add(&mut self.state, n, self.limits.max_state_bytes)
    }
    pub(super) fn prepay_k0(&mut self,l:tos_foundation::OperationBindingLimits)->Result<()> {
        self.tick(l.max_nodes)?;
        add(&mut self.bytes,l.max_envelope_bytes,self.limits.max_input_bytes)?;
        add(&mut self.nodes,l.max_nodes,self.limits.max_nodes)?;
        add(&mut self.refs,l.max_refs,self.limits.max_refs)?;
        if l.max_depth>self.limits.max_depth {return Err(Error::ResourceLimit);}Ok(())
    }
    pub(super) fn vec<T>(&mut self, count: usize) -> Result<Vec<T>> {
        if count > self.limits.max_items { return Err(Error::ResourceLimit); }
        self.state(size_of::<Vec<T>>().checked_add(count.checked_mul(size_of::<T>())
            .ok_or(Error::ResourceLimit)?).ok_or(Error::ResourceLimit)?)?;
        let mut out = Vec::new(); out.try_reserve_exact(count).map_err(|_| Error::ResourceLimit)?; Ok(out)
    }
    pub(super) fn push<T>(&mut self, v:&mut Vec<T>, x:T)->Result<()> {
        if v.len()>=self.limits.max_items {return Err(Error::ResourceLimit);}
        self.state(size_of::<T>())?;
        v.try_reserve_exact(1).map_err(|_|Error::ResourceLimit)?;v.push(x);Ok(())
    }
    pub(super) fn equal(&mut self,a:&Value,b:&Value)->Result<bool> {
        self.tick(1)?;
        match (a,b) {
            (Value::String(a),Value::String(b))=>self.same_text(a,b),
            (Value::Array(a),Value::Array(b))=>{
                if a.len()!=b.len(){return Ok(false);}for (a,b) in a.iter().zip(b){if !self.equal(a,b)?{return Ok(false);}}Ok(true)
            }
            (Value::Object(a),Value::Object(b))=>{
                if a.len()!=b.len(){return Ok(false);}for (k,v) in a {self.tick(k.len().checked_mul(b.len()).ok_or(Error::ResourceLimit)?)?;
                    let Some(w)=b.get(k) else{return Ok(false)};if !self.equal(v,w)?{return Ok(false);}}Ok(true)
            }
            (Value::Number(_),Value::Number(_))=>{
                for x in [a,b] {self.tick(self.limits.max_input_bytes.checked_mul(8).ok_or(Error::ResourceLimit)?)?;
                    let n=crate::source_bibliographic_render::encoded_size(x,self.limits.max_input_bytes).map_err(|_|Error::ResourceLimit)?;self.tick(n)?;}Ok(a==b)
            }
            _=>Ok(a==b),
        }
    }
    /// serde Value cannot contain duplicate keys. Supplied source owners must use
    /// their PublishedStrict parser before this borrowed-value API, as for the
    /// maintained metadata facade. We do not retroactively certify raw bytes.
    pub(super) fn admit(&mut self, v: &Value) -> Result<()> { self.walk(v,0) }
    fn walk(&mut self, v: &Value, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth { return Err(Error::ResourceLimit); }
        self.tick(1)?; add(&mut self.nodes,1,self.limits.max_nodes)?;
        match v {
            Value::Null => add(&mut self.bytes,4,self.limits.max_input_bytes)?,
            Value::Bool(_) => add(&mut self.bytes,5,self.limits.max_input_bytes)?,
            Value::Number(_) => {
                self.tick(self.limits.max_input_bytes.checked_mul(8).ok_or(Error::ResourceLimit)?)?;
                let n = crate::source_bibliographic_render::encoded_size(v,self.limits.max_input_bytes)
                    .map_err(|_| Error::ResourceLimit)?;
                self.tick(n)?; add(&mut self.bytes,n,self.limits.max_input_bytes)?;
            }
            Value::String(s) => {
                self.tick(s.len())?;
                let n=s.len().checked_mul(6).and_then(|n|n.checked_add(2)).ok_or(Error::ResourceLimit)?;
                add(&mut self.bytes,n,self.limits.max_input_bytes)?;
            }
            Value::Array(xs) => {
                add(&mut self.bytes,xs.len().checked_add(2).ok_or(Error::ResourceLimit)?,self.limits.max_input_bytes)?;
                for x in xs { self.walk(x,depth+1)?; }
            }
            Value::Object(xs) => {
                add(&mut self.bytes,2,self.limits.max_input_bytes)?;
                for (k,x) in xs {
                    self.tick(k.len())?;
                    let n=k.len().checked_mul(6).and_then(|n|n.checked_add(4)).ok_or(Error::ResourceLimit)?;
                    add(&mut self.bytes,n,self.limits.max_input_bytes)?; self.walk(x,depth+1)?;
                }
            }
        } Ok(())
    }
    pub(super) fn field<'v>(&mut self,v:&'v Value,k:&str)->Result<&'v Value> {
        let obj=v.as_object().ok_or(Error::Malformed("object"))?;
        // Charge an upper bound for the map lookup, independent of tree shape.
        for key in obj.keys() { self.tick(key.len().checked_add(k.len()+1).ok_or(Error::ResourceLimit)?)?; }
        obj.get(k).ok_or(Error::Malformed("missing field"))
    }
    pub(super) fn text<'v>(&mut self,v:&'v Value)->Result<&'v str> {
        let s=v.as_str().ok_or(Error::Malformed("UTF-8 string"))?;self.tick(s.len())?;
        if s.is_empty() { return Err(Error::Malformed("empty string")); } Ok(s)
    }
    pub(super) fn str_field<'v>(&mut self,v:&'v Value,k:&str)->Result<&'v str> {
        let x=self.field(v,k)?; self.text(x)
    }
    pub(super) fn array<'v>(&mut self,v:&'v Value)->Result<&'v [Value]> {
        let x=v.as_array().ok_or(Error::Malformed("array"))?;
        self.tick(x.len())?; if x.len()>self.limits.max_items {return Err(Error::ResourceLimit);} Ok(x)
    }
    pub(super) fn same_text(&mut self,a:&str,b:&str)->Result<bool> {
        self.tick(a.len().checked_add(b.len()).ok_or(Error::ResourceLimit)?)?; Ok(a==b)
    }
    pub(super) fn exact<'v>(&mut self,v:&'v Value)->Result<PweRef<'v>> {
        add(&mut self.refs,1,self.limits.max_refs)?;
        if v.as_object().map(|x|x.len())!=Some(3) {return Err(Error::Malformed("exact ref shape"));}
        let id=self.str_field(v,"id")?;
        if id.chars().any(char::is_control) {return Err(Error::Malformed("stable id"));}
        let ver=self.field(v,"version")?.as_u64().filter(|n|*n>0).ok_or(Error::Malformed("positive u64 version"))?;
        let d=self.str_field(v,"digest")?;
        let digest=Digest256::from_prefixed(d).map_err(|_|Error::Malformed("canonical digest"))?;
        Ok(PweRef{id,version:ver,digest})
    }
    pub(super) fn typed<'v>(&mut self,v:&'v Value)->Result<(PweRef<'v>,PweRef<'v>)> {
        if v.as_object().map(|x|x.len())!=Some(2) {return Err(Error::Malformed("TypedRef"));}
        let k=self.field(v,"kind_binding")?;let r=self.field(v,"record")?;
        Ok((self.exact(k)?,self.exact(r)?))
    }
    pub(super) fn bound<'v>(&mut self,v:&'v Value,slot:&'static str)->Result<&'v Value> {
        match self.str_field(v,"state")? {
            "bound" => {let x=self.field(v,"value")?;self.typed(x)?;Ok(x)},
            "unresolved" => Err(Error::NeedsBinding(slot)),_=>Err(Error::Malformed("binding state")),
        }
    }
    pub(super) fn digest(&mut self,v:&Value)->Result<Digest256> {
        // Existing bounded canonical owner codec; its temporary work/holders are
        // prepaid by this explicit conservative logical helper envelope. This is
        // not an allocator/RSS claim, and it does not include the retained input.
        let cap=self.limits.max_input_bytes;
        self.tick(cap.checked_mul(8).ok_or(Error::ResourceLimit)?)?;
        let n=crate::source_bibliographic_render::encoded_size(v,cap).map_err(|_|Error::ResourceLimit)?;
        let slots=size_of::<tos_foundation::JsonValue>().checked_add(size_of::<tos_foundation::JsonString>())
            .and_then(|x|x.checked_add(size_of::<Value>())).and_then(|x|x.checked_add(size_of::<(String,Value)>()))
            .and_then(|x|x.checked_mul(8)).and_then(|x|x.checked_add(64)).ok_or(Error::ResourceLimit)?;
        self.state(n.checked_mul(slots).ok_or(Error::ResourceLimit)?)?;
        self.tick(n.checked_mul(n).and_then(|x|x.checked_mul(8)).and_then(|x|x.checked_add(4_000_000)).ok_or(Error::ResourceLimit)?)?;
        let raw=crate::source_bibliographic_render::encode(v,cap).map_err(|_|Error::Upstream("canonical owner codec"))?;
        self.check()?;Ok(Digest256::of_bytes(&raw))
    }
    pub(super) fn record_matches(&mut self,record:&Value,r:PweRef<'_>,id:&str,version:&str)->Result<()> {
        let actual=self.str_field(record,id)?;
        if !self.same_text(actual,r.id)? || self.field(record,version)?.as_u64()!=Some(r.version)
            || self.digest(record)?!=r.digest {return Err(Error::ExactMismatch);} Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PweRef<'a> { pub(super) id:&'a str, pub(super) version:u64, pub(super) digest:Digest256 }
impl<'a> PweRef<'a> {pub fn id(self)->&'a str{self.id} pub fn version(self)->u64{self.version} pub fn digest(self)->Digest256{self.digest}}
