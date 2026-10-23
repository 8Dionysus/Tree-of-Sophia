//! ts-native-domain-payload-v1. Decimal strings are exact, canonical and bounded
//! by the existing i128/usize implementation profiles; no float conversion.
use crate::{Allen, CycleStep, DomainError, OrderOutcome, Premise, Rational,
    RelationImage, Span, TsCodomain, TsOutputValue, TsProfileBody, TsProjection};
use tos_foundation::{JsonString, JsonValue};
use super::session::{NativeTsSession, push};
type Result<T> = std::result::Result<T, DomainError>;

pub(crate) fn field<'a>(v: &'a JsonValue, name: &str, s: &mut NativeTsSession<'_>) -> Result<&'a JsonValue> {
    for (k,v) in v.as_object().ok_or(DomainError::Invalid("native TS object"))? {
        s.work(k.units().len().checked_add(1).ok_or(DomainError::ResourceLimit)?)?;
        if k.as_str() == Some(name) { return Ok(v); }
    }
    Err(DomainError::Invalid("native TS missing field"))
}
pub(crate) fn text<'a>(v: &'a JsonValue, s: &mut NativeTsSession<'_>) -> Result<&'a str> {
    let t=v.as_str().ok_or(DomainError::Invalid("native TS string"))?;s.work(t.len())?;Ok(t)
}
pub(super) fn closed(v: &JsonValue, keys: &[&str], s: &mut NativeTsSession<'_>) -> Result<()> {
    let entries=v.as_object().ok_or(DomainError::Invalid("native TS closed object"))?;
    if entries.len()!=keys.len() || keys.len()>8 { return Err(DomainError::Invalid("native TS closed fields")); }
    let mut seen=0u16;
    for (k,_) in entries {
        s.work(k.units().len().checked_add(keys.len()).ok_or(DomainError::ResourceLimit)?)?;
        let i=keys.iter().position(|n|k.as_str()==Some(*n)).ok_or(DomainError::Invalid("native TS unknown field"))?;
        if seen&(1<<i)!=0 { return Err(DomainError::Invalid("native TS duplicate field")); }
        seen|=1<<i;
    }
    Ok(())
}
fn array<'a>(v:&'a JsonValue)->Result<&'a [JsonValue]>{v.as_array().ok_or(DomainError::Invalid("native TS array"))}
fn boolean(v:&JsonValue)->Result<bool>{v.as_bool().ok_or(DomainError::Invalid("native TS bool"))}
fn decimal<'a>(v:&'a JsonValue, signed:bool, s:&mut NativeTsSession<'_>)->Result<&'a str>{
    let t=text(v,s)?;let digits=if signed {t.strip_prefix('-').unwrap_or(t)} else {t};
    if digits.is_empty() || !digits.bytes().all(|x|x.is_ascii_digit())
        || digits.len()>1&&digits.starts_with('0') || t=="-0" {
        return Err(DomainError::Invalid("native TS canonical decimal string"));
    }
    Ok(t)
}
fn index(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<usize>{
    decimal(v,false,s)?.parse().map_err(|_|DomainError::NumericRange)
}
fn rational(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Rational>{
    closed(v,&["n","d"],s)?;
    let n=decimal(field(v,"n",s)?,true,s)?.parse::<i128>().map_err(|_|DomainError::NumericRange)?;
    let d=decimal(field(v,"d",s)?,false,s)?.parse::<i128>().map_err(|_|DomainError::NumericRange)?;
    let q=Rational::new(n,d)?;
    if q.numerator()!=n||q.denominator()!=d {return Err(DomainError::Invalid("native TS non-normalized rational"));}
    Ok(q)
}
fn span(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Span>{
    closed(v,&["lower","upper","lower_closed","upper_closed"],s)?;
    Span::new(rational(field(v,"lower",s)?,s)?,rational(field(v,"upper",s)?,s)?,
        boolean(field(v,"lower_closed",s)?)?,boolean(field(v,"upper_closed",s)?)?)
}
fn label(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Allen>{
    Ok(match text(v,s)? {
        "Before"=>Allen::Before,"Meets"=>Allen::Meets,"Overlaps"=>Allen::Overlaps,
        "Starts"=>Allen::Starts,"During"=>Allen::During,"Finishes"=>Allen::Finishes,
        "Equals"=>Allen::Equals,"FinishedBy"=>Allen::FinishedBy,"Contains"=>Allen::Contains,
        "StartedBy"=>Allen::StartedBy,"OverlappedBy"=>Allen::OverlappedBy,
        "MetBy"=>Allen::MetBy,"After"=>Allen::After,
        _=>return Err(DomainError::Unsupported("native TS Allen label")),
    })
}
fn label_text(a:Allen)->&'static str{match a {
    Allen::Before=>"Before",Allen::Meets=>"Meets",Allen::Overlaps=>"Overlaps",Allen::Starts=>"Starts",
    Allen::During=>"During",Allen::Finishes=>"Finishes",Allen::Equals=>"Equals",Allen::FinishedBy=>"FinishedBy",
    Allen::Contains=>"Contains",Allen::StartedBy=>"StartedBy",Allen::OverlappedBy=>"OverlappedBy",Allen::MetBy=>"MetBy",Allen::After=>"After"}}
pub fn codomain_tag(c:TsCodomain)->&'static str{match c{TsCodomain::SpanMeet=>"SpanMeet",TsCodomain::AllenRelation=>"AllenRelation",
    TsCodomain::OrderSolution=>"OrderSolution",TsCodomain::RelationImage=>"RelationImage",TsCodomain::TopologyValue=>"TopologyValue"}}
pub fn decode_codomain(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<TsCodomain>{Ok(match text(v,s)?{
    "SpanMeet"=>TsCodomain::SpanMeet,"AllenRelation"=>TsCodomain::AllenRelation,"OrderSolution"=>TsCodomain::OrderSolution,
    "RelationImage"=>TsCodomain::RelationImage,"TopologyValue"=>TsCodomain::TopologyValue,
    _=>return Err(DomainError::Unsupported("native TS codomain")),})}
fn values(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Vec<Rational>>{
    let mut out=Vec::new();for x in array(v)? {let q=rational(x,s)?;push(s,&mut out,q)?;}Ok(out)
}
fn premise(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Premise>{
    match text(field(v,"kind",s)?,s)?{
        "ProperInterval"=>{closed(v,&["kind","interval"],s)?;Ok(Premise::ProperInterval(index(field(v,"interval",s)?,s)?))},
        "Constraint"=>{closed(v,&["kind","constraint"],s)?;Ok(Premise::Constraint(index(field(v,"constraint",s)?,s)?))},
        "AnchorOrder"=>{closed(v,&["kind","left_anchor","right_anchor"],s)?;Ok(Premise::AnchorOrder{
            left_anchor:index(field(v,"left_anchor",s)?,s)?,right_anchor:index(field(v,"right_anchor",s)?,s)?})},
        "Hypothesis"=>{closed(v,&["kind","hypothesis"],s)?;Ok(Premise::Hypothesis(index(field(v,"hypothesis",s)?,s)?))},
        _=>Err(DomainError::Unsupported("native TS cycle premise")),
    }
}
fn cycle(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<Vec<CycleStep>>{
    let mut out=Vec::new();for x in array(v)?{
        closed(x,&["from","to","strict","premise"],s)?;
        let step=CycleStep{from:index(field(x,"from",s)?,s)?,to:index(field(x,"to",s)?,s)?,
            strict:boolean(field(x,"strict",s)?)?,premise:premise(field(x,"premise",s)?,s)?};
        push(s,&mut out,step)?;
    }Ok(out)
}
fn order(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<OrderOutcome>{
    match text(field(v,"kind",s)?,s)?{
        "Witness"=>{closed(v,&["kind","endpoints"],s)?;Ok(OrderOutcome::Witness(values(field(v,"endpoints",s)?,s)?))},
        "StrictCycle"=>{closed(v,&["kind","steps"],s)?;Ok(OrderOutcome::StrictCycle(cycle(field(v,"steps",s)?,s)?))},
        _=>Err(DomainError::Unsupported("native TS order outcome")),
    }
}
fn image(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<RelationImage>{
    match text(field(v,"kind",s)?,s)?{
        "NoAdmissibleModel"=>{closed(v,&["kind","cycle"],s)?;Ok(RelationImage::NoAdmissibleModel(cycle(field(v,"cycle",s)?,s)?))},
        "Exact"=>{closed(v,&["kind","possible"],s)?;let mut possible=Vec::new();
            for x in array(field(v,"possible",s)?)?{closed(x,&["relation","endpoints"],s)?;
                let a=label(field(x,"relation",s)?,s)?;let w=values(field(x,"endpoints",s)?,s)?;
                push(s,&mut possible,(a,w))?;}
            Ok(RelationImage::Exact{possible})},
        _=>Err(DomainError::Unsupported("native TS relation image")),
    }
}
fn check_values(v:&[Rational],s:&mut NativeTsSession<'_>)->Result<()>{
    if v.is_empty()||v.len()%2!=0{return Err(DomainError::Invalid("native TS witness width"));}
    for x in v.chunks_exact(2){s.work(1)?;if x[0]>=x[1]{return Err(DomainError::Invalid("native TS proper endpoint pair"));}}Ok(())
}
fn check_cycle(v:&[CycleStep],s:&mut NativeTsSession<'_>)->Result<()>{
    if v.is_empty(){return Err(DomainError::Invalid("native TS empty cycle"));}let mut strict=false;
    for (i,x) in v.iter().enumerate(){s.work(1)?;
        if x.to!=v[(i+1)%v.len()].from||matches!(x.premise,Premise::Hypothesis(_)){
            return Err(DomainError::Invalid("native TS base cycle shape"));}strict|=x.strict;}
    if !strict{return Err(DomainError::Invalid("native TS nonstrict cycle"));}Ok(())
}
fn validate(v:&TsOutputValue,s:&mut NativeTsSession<'_>)->Result<()>{match v{
    TsOutputValue::OrderSolution(OrderOutcome::Witness(v))=>check_values(v,s),
    TsOutputValue::OrderSolution(OrderOutcome::StrictCycle(v))|TsOutputValue::RelationImage(RelationImage::NoAdmissibleModel(v))=>check_cycle(v,s),
    TsOutputValue::RelationImage(RelationImage::Exact{possible})=>{
        if possible.is_empty()||possible.len()>13{return Err(DomainError::Invalid("native TS exact relation image size"));}
        let width=possible[0].1.len();for (i,(a,v)) in possible.iter().enumerate(){check_values(v,s)?;
            if v.len()!=width{return Err(DomainError::Invalid("native TS joint witness width"));}
            for (b,_) in &possible[..i]{s.work(1)?;if a==b{return Err(DomainError::Invalid("native TS duplicate Allen image"));}}}Ok(())},
    _=>{s.work(1)?;Ok(())},
}}
pub fn decode_domain_value(c:TsCodomain,v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<TsOutputValue>{
    s.check()?;let out=match c{
        TsCodomain::SpanMeet=>TsOutputValue::SpanMeet(if matches!(v,JsonValue::Null){None}else{Some(span(v,s)?)}),
        TsCodomain::AllenRelation=>TsOutputValue::AllenRelation(label(v,s)?),
        TsCodomain::OrderSolution=>TsOutputValue::OrderSolution(order(v,s)?),
        TsCodomain::RelationImage=>TsOutputValue::RelationImage(image(v,s)?),
        TsCodomain::TopologyValue=>TsOutputValue::TopologyValue(boolean(v)?),
    };validate(&out,s)?;Ok(out)
}
pub fn decode_profile_body(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<TsProfileBody>{
    closed(v,&["projection","codomain"],s)?;
    if text(field(v,"projection",s)?,s)?!="DirectDomainValueV1"{
        return Err(DomainError::Unsupported("native TS projection"));}
    Ok(TsProfileBody{projection:TsProjection::DirectDomainValueV1,codomain:decode_codomain(field(v,"codomain",s)?,s)?})
}
/// Native /body decoding is distinct from the subsequent typed identity map.
pub fn decode_result_body(v:&JsonValue,s:&mut NativeTsSession<'_>)->Result<(TsCodomain,TsOutputValue)>{
    closed(v,&["codomain","value"],s)?;let c=decode_codomain(field(v,"codomain",s)?,s)?;
    Ok((c,decode_domain_value(c,field(v,"value",s)?,s)?))
}
pub fn typed_projection<'a>(p:&TsProfileBody,c:TsCodomain,v:&'a TsOutputValue,
    s:&mut NativeTsSession<'_>)->Result<&'a TsOutputValue>{
    if p.projection!=TsProjection::DirectDomainValueV1||p.codomain!=c{return Err(DomainError::Incompatible("native TS typed projection codomain"));}
    if value_codomain(v)!=c{return Err(DomainError::Incompatible("native TS actual typed value"));}
    validate(v,s)?;Ok(v)
}
pub fn value_codomain(v:&TsOutputValue)->TsCodomain{match v{
    TsOutputValue::SpanMeet(_)=>TsCodomain::SpanMeet,TsOutputValue::AllenRelation(_)=>TsCodomain::AllenRelation,
    TsOutputValue::OrderSolution(_)=>TsCodomain::OrderSolution,TsOutputValue::RelationImage(_)=>TsCodomain::RelationImage,
    TsOutputValue::TopologyValue(_)=>TsCodomain::TopologyValue}}

fn string(t:&str,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    s.work(t.len())?;s.requested_bytes(t.len().checked_mul(3).ok_or(DomainError::ResourceLimit)?)?;
    Ok(JsonValue::String(JsonString::from_utf8(t)))
}
fn object_vec(entries:Vec<(&'static str,JsonValue)>,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let mut out=Vec::new();for (k,v) in entries{
        let JsonValue::String(key)=string(k,s)? else{return Err(DomainError::Invalid("native TS key"))};
        push(s,&mut out,(key,v))?;
    }Ok(JsonValue::Object(out))
}
fn pair_object(a:(&'static str,JsonValue),b:(&'static str,JsonValue),s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let mut v=Vec::new();push(s,&mut v,a)?;push(s,&mut v,b)?;object_vec(v,s)
}
fn q_encode(q:Rational,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    // Formatting two fixed-width integers has a constant maximum of 80 bytes.
    s.requested_bytes(80)?;
    let n=string(&q.numerator().to_string(),s)?;let d=string(&q.denominator().to_string(),s)?;
    pair_object(("n",n),("d",d),s)
}
fn idx_encode(i:usize,s:&mut NativeTsSession<'_>)->Result<JsonValue>{s.requested_bytes(40)?;string(&i.to_string(),s)}
fn witness_encode(v:&[Rational],s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let mut out=Vec::new();for q in v{let x=q_encode(*q,s)?;push(s,&mut out,x)?;}Ok(JsonValue::Array(out))
}
fn premise_encode(p:Premise,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let mut v=Vec::new();let (kind,k,n)=match p{
        Premise::ProperInterval(i)=>("ProperInterval","interval",i),Premise::Constraint(i)=>("Constraint","constraint",i),
        Premise::Hypothesis(i)=>("Hypothesis","hypothesis",i),Premise::AnchorOrder{left_anchor,..}=>("AnchorOrder","left_anchor",left_anchor)};
    let tag=string(kind,s)?;push(s,&mut v,("kind",tag))?;let n=idx_encode(n,s)?;push(s,&mut v,(k,n))?;
    if let Premise::AnchorOrder{right_anchor,..}=p{let n=idx_encode(right_anchor,s)?;push(s,&mut v,("right_anchor",n))?;}
    object_vec(v,s)
}
fn cycle_encode(v:&[CycleStep],s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let mut out=Vec::new();for x in v{let mut e=Vec::new();
        let from=idx_encode(x.from,s)?;push(s,&mut e,("from",from))?;
        let to=idx_encode(x.to,s)?;push(s,&mut e,("to",to))?;
        push(s,&mut e,("strict",JsonValue::Bool(x.strict)))?;
        let p=premise_encode(x.premise,s)?;push(s,&mut e,("premise",p))?;
        let obj=object_vec(e,s)?;push(s,&mut out,obj)?;
    }Ok(JsonValue::Array(out))
}
pub fn encode_domain_value(v:&TsOutputValue,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    validate(v,s)?;Ok(match v{
        TsOutputValue::SpanMeet(None)=>JsonValue::Null,
        TsOutputValue::SpanMeet(Some(v))=>{let mut out=Vec::new();let lo=q_encode(v.lower(),s)?;push(s,&mut out,("lower",lo))?;
            let hi=q_encode(v.upper(),s)?;push(s,&mut out,("upper",hi))?;
            push(s,&mut out,("lower_closed",JsonValue::Bool(v.lower_closed())))?;
            push(s,&mut out,("upper_closed",JsonValue::Bool(v.upper_closed())))?;object_vec(out,s)?},
        TsOutputValue::AllenRelation(a)=>string(label_text(*a),s)?,
        TsOutputValue::TopologyValue(b)=>JsonValue::Bool(*b),
        TsOutputValue::OrderSolution(OrderOutcome::Witness(v))=>{let tag=string("Witness",s)?;let w=witness_encode(v,s)?;pair_object(("kind",tag),("endpoints",w),s)?},
        TsOutputValue::OrderSolution(OrderOutcome::StrictCycle(v))=>{let tag=string("StrictCycle",s)?;let w=cycle_encode(v,s)?;pair_object(("kind",tag),("steps",w),s)?},
        TsOutputValue::RelationImage(RelationImage::NoAdmissibleModel(v))=>{let tag=string("NoAdmissibleModel",s)?;let w=cycle_encode(v,s)?;pair_object(("kind",tag),("cycle",w),s)?},
        TsOutputValue::RelationImage(RelationImage::Exact{possible})=>{let mut out=Vec::new();for (a,w) in possible{
            let tag=string(label_text(*a),s)?;let w=witness_encode(w,s)?;let row=pair_object(("relation",tag),("endpoints",w),s)?;push(s,&mut out,row)?;}
            let tag=string("Exact",s)?;pair_object(("kind",tag),("possible",JsonValue::Array(out)),s)?},
    })
}
pub fn encode_result_body(v:&TsOutputValue,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let c=string(codomain_tag(value_codomain(v)),s)?;let value=encode_domain_value(v,s)?;
    pair_object(("codomain",c),("value",value),s)
}
pub fn encode_profile_body(v:&TsProfileBody,s:&mut NativeTsSession<'_>)->Result<JsonValue>{
    let projection=match v.projection{TsProjection::DirectDomainValueV1=>string("DirectDomainValueV1",s)?};let c=string(codomain_tag(v.codomain),s)?;
    pair_object(("projection",projection),("codomain",c),s)
}
