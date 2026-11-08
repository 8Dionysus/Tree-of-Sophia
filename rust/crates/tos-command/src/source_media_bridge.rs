//! Media bridges: one object's catalogue record and images, read from a
//! museum provider and returned as a `media-provider-observation` packet.
//!
//! A bridge reads over HTTPS from its provider's own hosts, fingerprints every
//! response and states what the provider says in the provider's own terms. It
//! writes nothing into the source home: admitting an Artifact, a visual
//! representation, rights or publication stays with their owners.

use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::Digest256;

type Result<T> = std::result::Result<T, String>;

const OBSERVATION_SCHEMA: &str = "media-provider-observation";
const AGENT_REF: &str = "software:tos-native-owner-command/acquisition/media.observe";
const IIIF_IMAGE_PROTOCOL: &str = "http://iiif.io/api/image";
const IIIF_IMAGE_2: &str = "http://iiif.io/api/image/2/context.json";
const IIIF_IMAGE_3: &str = "http://iiif.io/api/image/3/context.json";

struct Provider {
    id: &'static str,
    hosts: &'static [&'static str],
}

const RIJKSMUSEUM: Provider = Provider {
    id: "rijksmuseum",
    hosts: &["data.rijksmuseum.nl", "iiif.micr.io"],
};
const MET: Provider = Provider {
    id: "met",
    hosts: &["collectionapi.metmuseum.org"],
};
const PROVIDERS: &[&Provider] = &[&RIJKSMUSEUM, &MET];

struct Response {
    status: u16,
    final_url: String,
    media_type: Option<String>,
    body: Vec<u8>,
}

#[cfg(test)]
type TestFetch = Box<dyn Fn(&str) -> Result<Response>>;

#[cfg(test)]
thread_local! {
    static TEST_FETCH: RefCell<Option<TestFetch>> = const { RefCell::new(None) };
}

/// JSON request boundary of the `media` acquisition family.
pub fn invoke(request: &Value) -> Result<Value> {
    match request.get("operation").and_then(Value::as_str) {
        Some("media.describe") => Ok(describe()),
        Some("media.observe") => observe(request),
        Some("media.prepare-artifact") => crate::source_media_plant::prepare_artifact(request),
        Some("media.plant-representation") => {
            crate::source_media_plant::plant_representation(request)
        }
        Some(_) => Err("unsupported media acquisition operation".into()),
        None => Err("media request operation is missing".into()),
    }
}

fn describe() -> Value {
    let providers: Vec<Value> = PROVIDERS
        .iter()
        .map(|provider| json!({"provider":provider.id,"hosts":provider.hosts}))
        .collect();
    json!({
        "operations":["media.describe","media.observe","media.prepare-artifact","media.plant-representation"],
        "providers":providers,
        "observation_contract":format!("ToS/contracts/{OBSERVATION_SCHEMA}.schema.json"),
        "writes":{
            "media.describe":[],
            "media.observe":[],
            "media.prepare-artifact":["ToS/research-packets/media-observations/","ToS/source-witnesses/discovery/runs/","ToS/source-witnesses/artifacts/<tradition>/<site>/<identity>/rights.json","ToS/source-witnesses/discovery/provenance.jsonl"],
            "media.plant-representation":["ToS/source-witnesses/artifacts/<tradition>/<site>/<identity>/representations/<view>/","ToS/source-witnesses/discovery/provenance.jsonl"]
        },
    })
}

fn observe(request: &Value) -> Result<Value> {
    let root = absolute_directory(request, "repo_root")?;
    let provider_id = text(request, "provider")?;
    let provider = PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| format!("unsupported media provider: {provider_id}"))?;
    let object = text(request, "object")?.trim();
    if object.is_empty() || object.len() > 512 || object.chars().any(char::is_control) {
        return Err("media object must be 1..512 printable characters".into());
    }
    let budget = request
        .get("max_requests")
        .and_then(Value::as_u64)
        .filter(|budget| *budget > 0)
        .and_then(|budget| usize::try_from(budget).ok())
        .ok_or("max_requests must state the observation's positive request budget")?;
    let callback = match request.get("fetch_callback") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err("fetch_callback must be a boolean".into()),
    };
    let observed_at = utcnow()?;
    let mut observer = Observer {
        provider,
        callback,
        limit: record_limit()?,
        budget,
        requests: Vec::new(),
        warnings: Vec::new(),
    };
    let found = match provider.id {
        "rijksmuseum" => observe_rijksmuseum(&mut observer, object)?,
        "met" => observe_met(&mut observer, object)?,
        _ => return Err("unsupported media provider".into()),
    };
    if found.images.is_empty() {
        observer
            .warnings
            .push("the provider serves no image for this object".into());
    }
    if found.rights.is_empty() {
        observer
            .warnings
            .push("the provider states no rights for this object or its images".into());
    }
    let observation = json!({
        "$schema":format!("https://tree-of-sophia.local/ToS/contracts/{OBSERVATION_SCHEMA}.schema.json"),
        "schema_version":"tos_media_provider_observation_v1",
        "provider":provider.id,
        "requested_object":object,
        "observed_at":observed_at,
        "requests":observer.requests,
        "object":found.object,
        "rights":found.rights,
        "images":found.images,
        "warnings":observer.warnings,
        "authority":{
            "evidence_status":"provider_observation",
            "review_status":"unreviewed",
            "identity_admitted":false,
            "rights_decided":false,
            "source_text_admitted":false,
            "semantic_authority":false,
            "graph_authority":false,
            "canon_authority":false,
            "publication_authority":false
        },
        "maker":{"maker_type":"software","agent_ref":AGENT_REF}
    });
    crate::source_registry_acquisition::validate_json(&root, &observation, OBSERVATION_SCHEMA)?;
    Ok(json!({"observation":observation}))
}

struct Found {
    object: Value,
    rights: Vec<Value>,
    images: Vec<Value>,
}

struct Observer {
    provider: &'static Provider,
    callback: bool,
    /// Bytes one provider response may hold.
    limit: usize,
    /// Provider requests this observation may make.
    budget: usize,
    requests: Vec<Value>,
    warnings: Vec<String>,
}

impl Observer {
    fn allowed(&self, url: &str) -> bool {
        https_host(url).is_some_and(|host| self.provider.hosts.contains(&host))
    }

    /// One provider response, parsed as JSON and recorded with its fingerprint.
    fn json(&mut self, role: &str, url: &str) -> Result<(Value, String)> {
        if self.requests.len() >= self.budget {
            return Err(format!(
                "the observation needs more than its {} requests; raise max_requests",
                self.budget
            ));
        }
        if !self.allowed(url) {
            return Err(format!(
                "{} bridge refuses a URL outside its hosts: {url}",
                self.provider.id
            ));
        }
        let started_at = utcnow()?;
        let tick = Instant::now();
        let response = if self.callback {
            callback_fetch(url, self.limit)?
        } else {
            fetch(url, self.limit)?
        };
        let elapsed = tick.elapsed().as_secs_f64();
        if response.status != 200 {
            return Err(format!(
                "{} answered HTTP {} for {url}",
                self.provider.id, response.status
            ));
        }
        if !self.allowed(&response.final_url) {
            return Err(format!(
                "{} redirected outside its hosts: {}",
                self.provider.id, response.final_url
            ));
        }
        if response.body.is_empty() || response.body.len() > self.limit {
            return Err(format!(
                "{} response size is out of bounds: {url}",
                self.provider.id
            ));
        }
        let value: Value = serde_json::from_slice(&response.body).map_err(|error| {
            format!("{} response is not JSON: {url}: {error}", self.provider.id)
        })?;
        self.requests.push(json!({
            "role":role,
            "url":url,
            "final_url":response.final_url,
            "http_status":200,
            "media_type":response.media_type,
            "byte_size":response.body.len(),
            "sha256":Digest256::of_bytes(&response.body).to_hex(),
            "started_at":started_at,
            "elapsed_seconds":elapsed
        }));
        Ok((value, response.final_url))
    }

    /// The service an IIIF image URL belongs to, observed through its info.json.
    fn image_service(&mut self, service_id: &str) -> Result<Option<Value>> {
        let info_url = format!("{service_id}/info.json");
        if !self.allowed(&info_url) {
            self.warnings.push(format!(
                "image service outside the bridge's hosts is left unobserved: {service_id}"
            ));
            return Ok(None);
        }
        let checked_at = utcnow()?;
        let (info, final_url) = self.json("image-service-info", &info_url)?;
        let fingerprint = self
            .requests
            .last()
            .map(|request| (request["byte_size"].clone(), request["sha256"].clone()))
            .ok_or("image service request was not recorded")?;
        let service = parse_image_service(&info, &final_url, fingerprint, &checked_at)?;
        if service["service_id"].as_str() != Some(service_id) {
            self.warnings.push(format!(
                "info.json names its service {} for the image URL service {service_id}",
                service["service_id"].as_str().unwrap_or("")
            ));
        }
        Ok(Some(service))
    }
}

// ---------------------------------------------------------------------------
// Rijksmuseum: Linked Art records, object -> VisualItem -> DigitalObject -> IIIF.

fn observe_rijksmuseum(observer: &mut Observer, object: &str) -> Result<Found> {
    let (object_uri, searched_number) = rijks_object_uri(observer, object)?;
    let record_url = rijks_data_url(&object_uri)
        .ok_or_else(|| format!("not a Rijksmuseum object URI: {object_uri}"))?;
    let (record, record_final) = observer.json("object-record", &record_url)?;
    if record.get("id").and_then(Value::as_str) != Some(object_uri.as_str()) {
        return Err(format!("Rijksmuseum record does not describe {object_uri}"));
    }
    if record.get("type").and_then(Value::as_str) != Some("HumanMadeObject") {
        return Err(format!(
            "Rijksmuseum record {object_uri} is not a HumanMadeObject"
        ));
    }

    let mut identifiers = Vec::new();
    let mut titles = Vec::new();
    for entry in items(&record, "identified_by") {
        let Some(content) = nonempty(entry, "content") else {
            continue;
        };
        match entry.get("type").and_then(Value::as_str) {
            Some("Identifier") => {
                let scheme = items(entry, "classified_as")
                    .find_map(notation)
                    .map(|label| format!("Rijksmuseum {label}"))
                    .unwrap_or_else(|| "Rijksmuseum identifier".into());
                push_unique(&mut identifiers, json!({"scheme":scheme,"value":content}));
            }
            Some("Name") => {
                let (language, language_uri) = language_of(entry);
                let role = items(entry, "classified_as")
                    .find_map(notation)
                    .or_else(|| {
                        items(entry, "classified_as")
                            .find_map(|kind| nonempty(kind, "id").map(str::to_owned))
                    });
                push_unique(
                    &mut titles,
                    json!({"text":content,"language":language,"language_uri":language_uri,"role":role}),
                );
            }
            _ => {}
        }
    }
    // The search matches object numbers partially; the record must carry the
    // number exactly as it was asked for.
    if let Some(number) = searched_number {
        let exact = items(&record, "identified_by").any(|entry| {
            entry.get("type").and_then(Value::as_str) == Some("Identifier")
                && nonempty(entry, "content") == Some(number)
        });
        if !exact {
            return Err(format!(
                "Rijksmuseum search found {object_uri}, whose object number is not {number}"
            ));
        }
    }
    push_unique(
        &mut identifiers,
        json!({"scheme":"Rijksmuseum object URI","value":object_uri}),
    );
    for equivalent in items(&record, "equivalent") {
        if let Some(id) = nonempty(equivalent, "id") {
            let scheme = if id.starts_with("http://hdl.handle.net/")
                || id.starts_with("https://hdl.handle.net/")
            {
                "Handle"
            } else {
                "equivalent URI"
            };
            push_unique(&mut identifiers, json!({"scheme":scheme,"value":id}));
        }
    }

    let mut makers = Vec::new();
    let mut production_statements = Vec::new();
    let mut statement = None;
    let mut begin_year: Option<i64> = None;
    let mut end_year: Option<i64> = None;
    for production in items(&record, "produced_by") {
        let mut parts: Vec<&Value> = vec![production];
        parts.extend(items(production, "part"));
        for part in parts {
            let role = items(part, "technique").find_map(notation);
            for agent in items(part, "carried_out_by") {
                push_unique(
                    &mut makers,
                    json!({
                        "label":notation(agent),
                        "role":role,
                        "provider_uri":nonempty(agent,"id"),
                        "authority_uris":equivalent_uris(agent)
                    }),
                );
            }
            for statement in items(part, "referred_to_by") {
                if let Some(text) = language_text(statement) {
                    push_unique(&mut production_statements, text);
                }
            }
        }
        for span in items(production, "timespan") {
            if statement.is_none() {
                statement = preferred_content(items(span, "identified_by"));
            }
            if let Some(year) = nonempty(span, "begin_of_the_begin").and_then(year_of) {
                begin_year = Some(begin_year.map_or(year, |known| known.min(year)));
            }
            if let Some(year) = nonempty(span, "end_of_the_end").and_then(year_of) {
                end_year = Some(end_year.map_or(year, |known| known.max(year)));
            }
        }
    }
    let dating = json!({"statement":statement,"begin_year":begin_year,"end_year":end_year});

    let mut classifications = Vec::new();
    for kind in items(&record, "classified_as") {
        if let Some(label) = notation(kind) {
            push_unique(&mut classifications, Value::String(label));
        }
    }
    let mut materials = Vec::new();
    for material in items(&record, "made_of") {
        if let Some(label) = notation(material) {
            push_unique(&mut materials, Value::String(label));
        }
    }
    let mut dimensions = Vec::new();
    for dimension in items(&record, "dimension") {
        let value = dimension.get("value").and_then(decimal);
        let statement = preferred_content(items(dimension, "referred_to_by"));
        if value.is_none() && statement.is_none() {
            continue;
        }
        dimensions.push(json!({
            "kind":items(dimension,"classified_as").find_map(notation),
            "value":value,
            "unit":dimension.get("unit").and_then(notation),
            "unit_uri":dimension.get("unit").and_then(|unit| nonempty(unit,"id")),
            "statement":statement
        }));
    }
    let mut credit_lines = Vec::new();
    for statement in items(&record, "referred_to_by") {
        let credit = items(statement, "classified_as").any(|kind| {
            kind.get("id").and_then(Value::as_str) == Some("http://vocab.getty.edu/aat/300026687")
        });
        if credit {
            if let Some(text) = language_text(statement) {
                push_unique(&mut credit_lines, text);
            }
        }
    }

    let mut rights = Vec::new();
    let mut images = Vec::new();
    let mut depicted = Vec::new();
    let mut visual_item_uri = None;
    let visual_items = unique_ids(items(&record, "shows"));
    let mut seen_access = Vec::new();
    if visual_items.is_empty() {
        observer
            .warnings
            .push("the object record shows no VisualItem".into());
    }
    for item_uri in &visual_items {
        let url = rijks_data_url(item_uri)
            .ok_or_else(|| format!("not a Rijksmuseum VisualItem URI: {item_uri}"))?;
        let (item, item_final) = observer.json("visual-item-record", &url)?;
        if item.get("id").and_then(Value::as_str) != Some(item_uri.as_str()) {
            return Err(format!("Rijksmuseum record does not describe {item_uri}"));
        }
        visual_item_uri.get_or_insert_with(|| item_uri.clone());
        for right in items(&item, "subject_to") {
            let statement_uri = items(right, "classified_as")
                .find_map(|kind| nonempty(kind, "id").filter(|id| id.starts_with("http")))
                .map(str::to_owned);
            let label = preferred_content(items(right, "identified_by")).or_else(|| {
                items(right, "classified_as")
                    .find_map(|kind| nonempty(kind, "_label").map(str::to_owned))
            });
            let Some(value) = statement_uri.clone().or_else(|| label.clone()) else {
                continue;
            };
            push_unique(
                &mut rights,
                json!({
                    "scope":"image",
                    "provider_field":"subject_to",
                    "value":value,
                    "statement_uri":statement_uri,
                    "label":label,
                    "stated_in":item_final
                }),
            );
        }
        for subject in items(&item, "represents") {
            push_unique(
                &mut depicted,
                json!({
                    "label":notation(subject),
                    "kind":nonempty(subject,"type"),
                    "provider_uri":nonempty(subject,"id"),
                    "authority_uris":equivalent_uris(subject)
                }),
            );
        }
        let digital_objects = unique_ids(items(&item, "digitally_shown_by"));
        if digital_objects.is_empty() {
            observer.warnings.push(format!(
                "VisualItem {item_uri} is digitally shown by no DigitalObject"
            ));
        }
        for digital_uri in &digital_objects {
            let url = rijks_data_url(digital_uri)
                .ok_or_else(|| format!("not a Rijksmuseum DigitalObject URI: {digital_uri}"))?;
            let (digital, _) = observer.json("digital-object-record", &url)?;
            for access in items(&digital, "access_point") {
                let Some(access_url) = nonempty(access, "id") else {
                    continue;
                };
                if seen_access.contains(&access_url.to_owned()) {
                    continue;
                }
                seen_access.push(access_url.to_owned());
                let image = match iiif_service_of(access_url) {
                    Some(service_id) => match observer.image_service(&service_id)? {
                        Some(service) => {
                            json!({"form":"image_service","access_url":access_url,"image_service":service,"file_url":null})
                        }
                        None => {
                            json!({"form":"file","access_url":access_url,"image_service":null,"file_url":access_url})
                        }
                    },
                    None if https_host(access_url).is_some() => {
                        json!({"form":"file","access_url":access_url,"image_service":null,"file_url":access_url})
                    }
                    None => {
                        observer.warnings.push(format!(
                            "access point is not an HTTPS image address: {access_url}"
                        ));
                        continue;
                    }
                };
                images.push(image);
            }
        }
    }
    assign_roles(&mut images);

    let object = json!({
        "provider_object_uri":object_uri,
        "record_url":object_uri,
        "api_url":record_final,
        "identifiers":identifiers,
        "titles":titles,
        "makers":makers,
        "production_statements":production_statements,
        "dating":dating,
        "classifications":classifications,
        "materials":materials,
        "dimensions":dimensions,
        "credit_lines":credit_lines,
        "repository":"Rijksmuseum, Amsterdam",
        "depicted":depicted,
        "visual_item_uri":visual_item_uri
    });
    Ok(Found {
        object,
        rights,
        images,
    })
}

/// The object URI, and the object number when the URI was found by search.
fn rijks_object_uri<'a>(
    observer: &mut Observer,
    object: &'a str,
) -> Result<(String, Option<&'a str>)> {
    if let Some(id) = object.strip_prefix("https://id.rijksmuseum.nl/") {
        if all_digits(id) {
            return Ok((object.to_owned(), None));
        }
        return Err(format!("not a Rijksmuseum object URI: {object}"));
    }
    if all_digits(object) {
        return Ok((format!("https://id.rijksmuseum.nl/{object}"), None));
    }
    if !object
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | ' ' | '(' | ')' | '/'))
    {
        return Err(format!("not a Rijksmuseum object number: {object}"));
    }
    let url = format!(
        "https://data.rijksmuseum.nl/search/collection?objectNumber={}",
        percent_encode(object)
    );
    let (page, _) = observer.json("search", &url)?;
    let found: Vec<&str> = items(&page, "orderedItems")
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("HumanMadeObject"))
        .filter_map(|item| nonempty(item, "id"))
        .collect();
    let more = page.get("next").is_some();
    match found.as_slice() {
        [uri]
            if !more
                && uri
                    .strip_prefix("https://id.rijksmuseum.nl/")
                    .is_some_and(all_digits) =>
        {
            Ok(((*uri).to_owned(), Some(object)))
        }
        [] => Err(format!("Rijksmuseum knows no object numbered {object}")),
        [_] if !more => Err(format!(
            "Rijksmuseum search returned an unexpected identifier for {object}"
        )),
        many => Err(format!(
            "Rijksmuseum search matches {}{} objects for {object}, taking the number partially; name the object by its URI, for example {}",
            many.len(),
            if more { " or more" } else { "" },
            many.iter().take(3).copied().collect::<Vec<_>>().join(", ")
        )),
    }
}

/// The Linked Art record of a Rijksmuseum URI, served by the data host.
fn rijks_data_url(uri: &str) -> Option<String> {
    let id = uri.strip_prefix("https://id.rijksmuseum.nl/")?;
    all_digits(id).then(|| format!("https://data.rijksmuseum.nl/{id}?_profile=la-framed"))
}

// ---------------------------------------------------------------------------
// The Met: the Collection API object record and its image files.

fn observe_met(observer: &mut Observer, object: &str) -> Result<Found> {
    if !all_digits(object) {
        return Err(format!(
            "a Met object is named by its numeric objectID: {object}"
        ));
    }
    let api_url =
        format!("https://collectionapi.metmuseum.org/public/collection/v1/objects/{object}");
    let (record, record_final) = observer.json("object-record", &api_url)?;
    let object_id = record
        .get("objectID")
        .and_then(|value| value.as_u64().map(|id| id.to_string()))
        .ok_or("Met record has no objectID")?;
    if object_id != object.trim_start_matches('0') {
        return Err(format!("Met record does not describe object {object}"));
    }
    let record_url = nonempty(&record, "objectURL")
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned)
        .unwrap_or_else(|| format!("https://www.metmuseum.org/art/collection/search/{object_id}"));

    let mut identifiers = vec![json!({"scheme":"Met object ID","value":object_id})];
    if let Some(number) = nonempty(&record, "accessionNumber") {
        identifiers.push(json!({"scheme":"Met object number","value":number}));
    }
    for (scheme, key) in [("Wikidata", "objectWikidata_URL")] {
        if let Some(uri) = nonempty(&record, key) {
            identifiers.push(json!({"scheme":scheme,"value":uri}));
        }
    }
    let titles: Vec<Value> = nonempty(&record, "title")
        .map(|title| json!({"text":title,"language":null,"language_uri":null,"role":null}))
        .into_iter()
        .collect();

    let mut makers = Vec::new();
    for constituent in items(&record, "constituents") {
        let authority: Vec<Value> = ["constituentULAN_URL", "constituentWikidata_URL"]
            .iter()
            .filter_map(|key| nonempty(constituent, key))
            .filter(|uri| uri.starts_with("http"))
            .map(|uri| Value::String(uri.to_owned()))
            .collect();
        push_unique(
            &mut makers,
            json!({"label":nonempty(constituent,"name"),"role":nonempty(constituent,"role"),"provider_uri":null,"authority_uris":authority}),
        );
    }
    if makers.is_empty() {
        if let Some(name) = nonempty(&record, "artistDisplayName") {
            let authority: Vec<Value> = ["artistULAN_URL", "artistWikidata_URL"]
                .iter()
                .filter_map(|key| nonempty(&record, key))
                .filter(|uri| uri.starts_with("http"))
                .map(|uri| Value::String(uri.to_owned()))
                .collect();
            makers.push(json!({"label":name,"role":nonempty(&record,"artistRole"),"provider_uri":null,"authority_uris":authority}));
        }
    }

    let statement = nonempty(&record, "objectDate");
    let year = |key: &str| record.get(key).and_then(Value::as_i64);
    let dating = match statement {
        Some(statement) => {
            json!({"statement":statement,"begin_year":year("objectBeginDate"),"end_year":year("objectEndDate")})
        }
        None => json!({"statement":null,"begin_year":null,"end_year":null}),
    };
    let mut classifications = Vec::new();
    for key in ["classification", "objectName"] {
        if let Some(label) = nonempty(&record, key) {
            push_unique(&mut classifications, Value::String(label.to_owned()));
        }
    }
    let materials: Vec<Value> = nonempty(&record, "medium")
        .map(|medium| Value::String(medium.to_owned()))
        .into_iter()
        .collect();
    let mut dimensions = Vec::new();
    if let Some(statement) = nonempty(&record, "dimensions") {
        dimensions.push(
            json!({"kind":null,"value":null,"unit":null,"unit_uri":null,"statement":statement}),
        );
    }
    for measurement in items(&record, "measurements") {
        let element = nonempty(measurement, "elementName");
        let Some(values) = measurement
            .get("elementMeasurements")
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (axis, value) in values {
            let Some(value) = decimal(value) else {
                continue;
            };
            let kind = match element {
                Some(element) => format!("{element} {}", axis.to_lowercase()),
                None => axis.to_lowercase(),
            };
            dimensions.push(
                json!({"kind":kind,"value":value,"unit":"cm","unit_uri":null,"statement":null}),
            );
        }
    }
    let credit_lines: Vec<Value> = nonempty(&record, "creditLine")
        .map(|credit| json!({"text":credit,"language":null,"language_uri":null}))
        .into_iter()
        .collect();
    let mut depicted = Vec::new();
    for tag in items(&record, "tags") {
        let authority: Vec<Value> = ["AAT_URL", "Wikidata_URL"]
            .iter()
            .filter_map(|key| nonempty(tag, key))
            .filter(|uri| uri.starts_with("http"))
            .map(|uri| Value::String(uri.to_owned()))
            .collect();
        push_unique(
            &mut depicted,
            json!({"label":nonempty(tag,"term"),"kind":"tag","provider_uri":null,"authority_uris":authority}),
        );
    }

    let mut rights = Vec::new();
    if let Some(public) = record.get("isPublicDomain").and_then(Value::as_bool) {
        rights.push(json!({"scope":"image","provider_field":"isPublicDomain","value":public.to_string(),"statement_uri":null,"label":null,"stated_in":record_final}));
    }
    if let Some(text) = nonempty(&record, "rightsAndReproduction") {
        rights.push(json!({"scope":"object","provider_field":"rightsAndReproduction","value":text,"statement_uri":null,"label":null,"stated_in":record_final}));
    }

    let mut images = Vec::new();
    let primary = nonempty(&record, "primaryImage").map(str::to_owned);
    let additional = items(&record, "additionalImages")
        .filter_map(Value::as_str)
        .map(str::to_owned);
    for url in primary.into_iter().chain(additional) {
        if https_host(&url).is_none() {
            observer
                .warnings
                .push(format!("image address is not HTTPS: {url}"));
            continue;
        }
        images.push(json!({"form":"file","access_url":url,"image_service":null,"file_url":url}));
    }
    assign_roles(&mut images);

    let object = json!({
        "provider_object_uri":record_url,
        "record_url":record_url,
        "api_url":record_final,
        "identifiers":identifiers,
        "titles":titles,
        "makers":makers,
        "production_statements":[],
        "dating":dating,
        "classifications":classifications,
        "materials":materials,
        "dimensions":dimensions,
        "credit_lines":credit_lines,
        "repository":nonempty(&record,"repository"),
        "depicted":depicted,
        "visual_item_uri":null
    });
    Ok(Found {
        object,
        rights,
        images,
    })
}

// ---------------------------------------------------------------------------
// IIIF Image API.

/// The service of an IIIF image request URL:
/// `{service}/{region}/{size}/{rotation}/{quality}.{format}`.
fn iiif_service_of(url: &str) -> Option<String> {
    let url = url.split(['?', '#']).next()?;
    let mut parts = url.rsplitn(5, '/');
    let quality_format = parts.next()?;
    let rotation = parts.next()?;
    let size = parts.next()?;
    let region = parts.next()?;
    let service = parts.next()?;
    let (quality, format) = quality_format.split_once('.')?;
    let word = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric());
    let number =
        |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let region_ok = matches!(region, "full" | "square") || {
        let parts: Vec<&str> = region
            .strip_prefix("pct:")
            .unwrap_or(region)
            .split(',')
            .collect();
        parts.len() == 4 && parts.iter().all(|part| number(part))
    };
    let size_ok = matches!(size, "full" | "max" | "^max")
        || size
            .trim_start_matches('^')
            .trim_start_matches('!')
            .strip_prefix("pct:")
            .map_or_else(
                || {
                    let body = size.trim_start_matches('^').trim_start_matches('!');
                    body.contains(',')
                        && body.split(',').all(|part| part.is_empty() || number(part))
                },
                number,
            );
    let rotation_ok = number(rotation.trim_start_matches('!'));
    let service_ok = https_host(service).is_some()
        && service
            .strip_prefix("https://")
            .is_some_and(|rest| rest.contains('/') && !rest.ends_with('/'));
    (word(quality) && word(format) && region_ok && size_ok && rotation_ok && service_ok)
        .then(|| service.to_owned())
}

fn parse_image_service(
    info: &Value,
    info_url: &str,
    fingerprint: (Value, Value),
    checked_at: &str,
) -> Result<Value> {
    let contexts: Vec<&str> = match info.get("@context") {
        Some(Value::String(context)) => vec![context.as_str()],
        Some(Value::Array(contexts)) => contexts.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let version = if contexts.contains(&IIIF_IMAGE_3) {
        "3"
    } else if contexts.contains(&IIIF_IMAGE_2) {
        "2"
    } else {
        return Err(format!(
            "info.json names no IIIF Image API 2 or 3 context: {info_url}"
        ));
    };
    if let Some(protocol) = info.get("protocol").and_then(Value::as_str) {
        if protocol != IIIF_IMAGE_PROTOCOL {
            return Err(format!("info.json names another protocol: {info_url}"));
        }
    }
    let service_id = info
        .get(if version == "3" { "id" } else { "@id" })
        .and_then(Value::as_str)
        .filter(|id| https_host(id).is_some())
        .ok_or_else(|| format!("info.json has no HTTPS service id: {info_url}"))?;
    let dimension = |key: &str| {
        info.get(key)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("info.json has no positive {key}: {info_url}"))
    };
    let width = dimension("width")?;
    let height = dimension("height")?;

    // Version 2 states its level first and its limits and features in the
    // profile objects that follow; version 3 states them at the top level.
    let (profile, extras): (String, Vec<&Value>) = match info.get("profile") {
        Some(Value::String(profile)) => (profile.clone(), vec![info]),
        Some(Value::Array(entries)) => {
            let level = entries
                .iter()
                .find_map(Value::as_str)
                .ok_or_else(|| format!("info.json profile names no level: {info_url}"))?;
            let mut extras: Vec<&Value> =
                entries.iter().filter(|entry| entry.is_object()).collect();
            extras.push(info);
            (level.to_owned(), extras)
        }
        _ => return Err(format!("info.json has no profile: {info_url}")),
    };
    let limit = |key: &str| {
        extras
            .iter()
            .find_map(|entry| entry.get(key).and_then(Value::as_u64))
            .filter(|value| *value > 0)
    };
    let mut formats = Vec::new();
    let mut qualities = Vec::new();
    for entry in &extras {
        for key in ["formats", "extraFormats"] {
            for format in items(entry, key).filter_map(Value::as_str) {
                push_unique(&mut formats, Value::String(format.to_owned()));
            }
        }
        for key in ["qualities", "extraQualities"] {
            for quality in items(entry, key).filter_map(Value::as_str) {
                push_unique(&mut qualities, Value::String(quality.to_owned()));
            }
        }
    }
    let mut tiles = Vec::new();
    for tile in items(info, "tiles") {
        let Some(tile_width) = tile.get("width").and_then(Value::as_u64).filter(|w| *w > 0) else {
            continue;
        };
        let tile_height = tile
            .get("height")
            .and_then(Value::as_u64)
            .filter(|h| *h > 0)
            .unwrap_or(tile_width);
        let mut scale_factors = Vec::new();
        for factor in items(tile, "scaleFactors")
            .filter_map(Value::as_u64)
            .filter(|f| *f > 0)
        {
            push_unique(&mut scale_factors, json!(factor));
        }
        if scale_factors.is_empty() {
            continue;
        }
        tiles.push(json!({"width":tile_width,"height":tile_height,"scale_factors":scale_factors}));
    }
    let (byte_size, sha256) = fingerprint;
    Ok(json!({
        "protocol":IIIF_IMAGE_PROTOCOL,
        "api_version":version,
        "service_id":service_id,
        "profile":profile,
        "width_pixels":width,
        "height_pixels":height,
        "max_area":limit("maxArea"),
        "max_width":limit("maxWidth"),
        "max_height":limit("maxHeight"),
        "tiles":tiles,
        "formats":formats,
        "qualities":qualities,
        "info_fingerprint":{"url":info_url,"byte_size":byte_size,"sha256":sha256,"checked_at":checked_at}
    }))
}

// ---------------------------------------------------------------------------
// Transport.

fn fetch(url: &str, limit: usize) -> Result<Response> {
    #[cfg(test)]
    if let Some(result) = TEST_FETCH.with(|fetch| fetch.borrow().as_ref().map(|fetch| fetch(url))) {
        return result;
    }
    const MARKER: &[u8] = b"\n__TOS_MEDIA_META__";
    let write_out = "\n__TOS_MEDIA_META__%{http_code}\n%{url_effective}\n%{content_type}";
    let args = [
        "-q",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--max-redirs",
        "10",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-time",
        "45",
        "--user-agent",
        "Tree-of-Sophia-media-bridge/1",
        "--header",
        "Accept: application/ld+json, application/json;q=0.9",
        "--write-out",
        write_out,
        "--url",
        url,
    ];
    let cancel = AtomicBool::new(false);
    let raw = crate::source_text_owner_ocr::bounded_process(
        "/usr/bin/curl",
        &args,
        None,
        limit + 4096,
        Instant::now() + Duration::from_secs(50),
        &cancel,
    )
    .map_err(|error| format!("media provider fetch failed: {url}: {error:?}"))?;
    let position = raw
        .windows(MARKER.len())
        .rposition(|window| window == MARKER)
        .ok_or("media provider response metadata is missing")?;
    let metadata = std::str::from_utf8(&raw[position + MARKER.len()..])
        .map_err(|_| "media provider response metadata is not UTF-8")?;
    let mut lines = metadata.splitn(3, '\n');
    let status = lines
        .next()
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or("media provider HTTP status is invalid")?;
    let final_url = lines
        .next()
        .filter(|url| !url.is_empty())
        .ok_or("media provider final URL is missing")?
        .to_owned();
    let media_type = lines
        .next()
        .map(str::trim)
        .filter(|media_type| !media_type.is_empty())
        .map(str::to_owned);
    Ok(Response {
        status,
        final_url,
        media_type,
        body: raw[..position].to_vec(),
    })
}

/// Bytes supplied by the caller over the acquisition wire, for operators who
/// fetch through their own transport. The caller answers for the URL as asked.
fn callback_fetch(url: &str, limit: usize) -> Result<Response> {
    let body =
        crate::source_acquisition_batch::fetch_request(&json!({"url":url,"max_bytes":limit}))?;
    Ok(Response {
        status: 200,
        final_url: url.to_owned(),
        media_type: None,
        body,
    })
}

// ---------------------------------------------------------------------------
// Small readers.

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("media request field {key} is missing"))
}

fn absolute_directory(value: &Value, key: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text(value, key)?);
    if !path.is_absolute() || !Path::new(&path).is_dir() {
        return Err(format!("{key} must be an existing absolute directory"));
    }
    Ok(path)
}

/// The values of a Linked Art property: JSON-LD compaction gives one value as
/// itself and several as an array, and the providers use both.
fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value.get(key).into_iter().flat_map(|value| match value {
        Value::Array(values) => values.as_slice(),
        Value::Null => &[],
        value => std::slice::from_ref(value),
    })
}

fn nonempty<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The `id`s of a Linked Art reference list, each once, in order.
fn unique_ids<'a>(references: impl Iterator<Item = &'a Value>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in references.filter_map(|reference| nonempty(reference, "id")) {
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    ids
}

fn push_unique(list: &mut Vec<Value>, value: Value) {
    if !list.contains(&value) {
        list.push(value);
    }
}

fn all_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn https_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty() && !host.contains(['@', ':']) && !rest.contains(char::is_whitespace))
        .then_some(host)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The label a Linked Art reference carries: its English notation, else its
/// Dutch one, else any notation or `_label`.
fn notation(value: &Value) -> Option<String> {
    for language in ["en", "nl"] {
        let found = items(value, "notation").find_map(|entry| {
            (entry.get("@language").and_then(Value::as_str) == Some(language))
                .then(|| nonempty(entry, "@value"))
                .flatten()
        });
        if let Some(found) = found {
            return Some(found.to_owned());
        }
    }
    items(value, "notation")
        .find_map(|entry| {
            nonempty(entry, "@value").or_else(|| {
                entry
                    .as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
            })
        })
        .or_else(|| nonempty(value, "_label"))
        .map(str::to_owned)
}

fn equivalent_uris(value: &Value) -> Vec<Value> {
    let mut uris = Vec::new();
    for equivalent in items(value, "equivalent") {
        if let Some(id) = nonempty(equivalent, "id").filter(|id| id.starts_with("http")) {
            push_unique(&mut uris, Value::String(id.to_owned()));
        }
    }
    uris
}

/// Getty AAT language concepts the Rijksmuseum records use.
fn aat_language(uri: &str) -> Option<&'static str> {
    match uri {
        "http://vocab.getty.edu/aat/300388256" => Some("nl"),
        "http://vocab.getty.edu/aat/300388277" => Some("en"),
        _ => None,
    }
}

fn language_of(value: &Value) -> (Option<&'static str>, Option<String>) {
    let uri = items(value, "language").find_map(|language| nonempty(language, "id"));
    (uri.and_then(aat_language), uri.map(str::to_owned))
}

fn language_text(value: &Value) -> Option<Value> {
    let text = nonempty(value, "content")?;
    let (language, language_uri) = language_of(value);
    Some(json!({"text":text,"language":language,"language_uri":language_uri}))
}

/// The English content of a set of statements, else the first one.
fn preferred_content<'a>(entries: impl Iterator<Item = &'a Value>) -> Option<String> {
    let entries: Vec<&Value> = entries.collect();
    entries
        .iter()
        .find(|entry| language_of(entry).0 == Some("en"))
        .or_else(|| entries.first())
        .and_then(|entry| nonempty(entry, "content"))
        .map(str::to_owned)
}

/// The year of an ISO 8601 instant as written, astronomical numbering kept.
fn year_of(instant: &str) -> Option<i64> {
    let (sign, rest) = match instant.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, instant.strip_prefix('+').unwrap_or(instant)),
    };
    let digits = rest.split('-').next()?;
    if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok().map(|year| sign * year)
}

/// A decimal value as the provider wrote it.
fn decimal(value: &Value) -> Option<String> {
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.trim().to_owned(),
        _ => return None,
    };
    let unsigned = text.strip_prefix('-').unwrap_or(&text);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, "0"));
    (!whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && !fraction.is_empty()
        && fraction.bytes().all(|b| b.is_ascii_digit()))
    .then_some(text)
}

fn assign_roles(images: &mut [Value]) {
    for (index, image) in images.iter_mut().enumerate() {
        if let Some(image) = image.as_object_mut() {
            let mut ordered = Map::new();
            ordered.insert(
                "role".into(),
                Value::String(if index == 0 { "primary" } else { "additional" }.into()),
            );
            for (key, value) in std::mem::take(image) {
                ordered.insert(key, value);
            }
            *image = ordered;
        }
    }
}

/// The acquisition record limit of the deployment profile, applied to every
/// provider response.
fn record_limit() -> Result<usize> {
    use tos_foundation::deployment_profile::{
        DEFAULT_DEPLOYMENT_PROFILE, parse_deployment_profile,
    };
    let profile = parse_deployment_profile(DEFAULT_DEPLOYMENT_PROFILE)
        .map_err(|error| format!("deployment profile: {error}"))?;
    usize::try_from(profile.admission.acquisition.max_record_bytes)
        .map_err(|_| "deployment profile acquisition record limit exceeds this host".into())
}

fn utcnow() -> Result<String> {
    crate::source_serialization::instant()
        .map_err(|error| format!("native clock unavailable: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn repo_root() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn with_responses<T>(responses: Vec<(&str, Value)>, run: impl FnOnce() -> T) -> T {
        let table: HashMap<String, Vec<u8>> = responses
            .into_iter()
            .map(|(url, body)| (url.to_owned(), serde_json::to_vec(&body).unwrap()))
            .collect();
        TEST_FETCH.with(|fetch| {
            *fetch.borrow_mut() = Some(Box::new(move |url: &str| {
                table
                    .get(url)
                    .map(|body| Response {
                        status: 200,
                        final_url: url.to_owned(),
                        media_type: Some("application/ld+json".into()),
                        body: body.clone(),
                    })
                    .ok_or_else(|| format!("unexpected fixture request: {url}"))
            }));
        });
        let result = run();
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        result
    }

    const EN: &str = "http://vocab.getty.edu/aat/300388277";
    const NL: &str = "http://vocab.getty.edu/aat/300388256";

    /// The Night Watch as the Rijksmuseum serves it, reduced to the fields the
    /// bridge reads; structure and values follow the live records of
    /// 2026-11-08.
    fn rijks_responses() -> Vec<(&'static str, Value)> {
        vec![
            (
                "https://data.rijksmuseum.nl/search/collection?objectNumber=SK-C-5",
                json!({"type":"OrderedCollectionPage","orderedItems":[{"id":"https://id.rijksmuseum.nl/200107928","type":"HumanMadeObject"}]}),
            ),
            (
                "https://data.rijksmuseum.nl/200107928?_profile=la-framed",
                json!({
                    "id":"https://id.rijksmuseum.nl/200107928","type":"HumanMadeObject",
                    "identified_by":[
                        {"type":"Identifier","classified_as":[{"id":"http://vocab.getty.edu/aat/300312355","type":"Type"},{"id":"https://id.rijksmuseum.nl/22015218","type":"Type","notation":[{"@language":"en","@value":"object number"},{"@language":"nl","@value":"objectnummer"}]}],"content":"SK-C-5"},
                        {"type":"Name","classified_as":[{"id":"http://vocab.getty.edu/aat/300404670","type":"Type"}],"content":"De Nachtwacht","language":[{"id":NL,"type":"Language"}]},
                        {"type":"Name","classified_as":[{"id":"https://id.rijksmuseum.nl/22015528","type":"Type","notation":[{"@language":"en","@value":"former title"}]}],"content":"The company of Captain Frans Banninck Cocq","language":[{"id":EN,"type":"Language"}]}
                    ],
                    "equivalent":[{"id":"http://hdl.handle.net/10934/RM0001.COLLECT.5216","type":"HumanMadeObject"}],
                    "produced_by":{"type":"Production","part":[{"type":"Production",
                        "carried_out_by":[{"id":"https://id.rijksmuseum.nl/2103429","type":"Person","equivalent":[{"id":"http://www.wikidata.org/entity/Q5598","type":"Person"}],"notation":[{"@language":"en","@value":"Rembrandt van Rijn"}]}],
                        "technique":[{"id":"https://id.rijksmuseum.nl/2202217","type":"Type","notation":[{"@language":"en","@value":"painter"},{"@language":"nl","@value":"schilder"}]}],
                        "referred_to_by":[{"type":"LinguisticObject","content":"painter: Rembrandt van Rijn, Amsterdam","language":[{"id":EN,"type":"Language"}]}]}],
                        "timespan":{"type":"TimeSpan","begin_of_the_begin":"1642-01-01T00:00:00Z","end_of_the_end":"1642-12-31T23:59:59Z","identified_by":[{"type":"Name","content":"1642","language":[{"id":EN,"type":"Language"}]}]}},
                    "classified_as":[{"id":"https://id.rijksmuseum.nl/2208","type":"Type","notation":[{"@language":"en","@value":"painting"},{"@language":"nl","@value":"schilderij"}]}],
                    "made_of":[{"id":"https://id.rijksmuseum.nl/22010","type":"Material","notation":[{"@language":"en","@value":"canvas"}]}],
                    "dimension":[{"type":"Dimension","classified_as":[{"id":"https://id.rijksmuseum.nl/22011","type":"Type","notation":[{"@language":"en","@value":"height"}]}],"unit":{"id":"http://vocab.getty.edu/aat/300379098","type":"MeasurementUnit"},"value":"379.5"}],
                    "referred_to_by":[{"type":"LinguisticObject","classified_as":[{"id":"http://vocab.getty.edu/aat/300026687","type":"Type"}],"content":"On loan from the City of Amsterdam","language":[{"id":EN,"type":"Language"}]}],
                    "shows":[{"id":"https://id.rijksmuseum.nl/202107928","type":"VisualItem"}]
                }),
            ),
            (
                "https://data.rijksmuseum.nl/202107928?_profile=la-framed",
                json!({
                    "id":"https://id.rijksmuseum.nl/202107928","type":"VisualItem",
                    "digitally_shown_by":[{"id":"https://id.rijksmuseum.nl/500711199912110510799100","type":"DigitalObject"}],
                    "represents":[{"id":"https://id.rijksmuseum.nl/21051134","type":"Person","equivalent":[{"id":"http://www.wikidata.org/entity/Q935192","type":"Person"}]}],
                    "subject_to":[{"type":"Right","classified_as":[{"id":"https://creativecommons.org/publicdomain/mark/1.0/","type":"Type","_label":"Public Domain"}],"identified_by":[{"type":"Name","content":"Public Domain","language":[{"id":EN,"type":"Language"}]}]}]
                }),
            ),
            (
                "https://data.rijksmuseum.nl/500711199912110510799100?_profile=la-framed",
                json!({"id":"https://id.rijksmuseum.nl/500711199912110510799100","type":"DigitalObject","access_point":[{"id":"https://iiif.micr.io/PJEZO/full/max/0/default.jpg","type":"DigitalObject"}]}),
            ),
            (
                "https://iiif.micr.io/PJEZO/info.json",
                json!({"@context":"http://iiif.io/api/image/3/context.json","id":"https://iiif.micr.io/PJEZO","type":"ImageService3","protocol":"http://iiif.io/api/image","profile":"level2","qualities":["default","gray","color"],"formats":["jpg","png","webp"],"tiles":[{"scaleFactors":[1,2,4,8,16],"width":1024,"height":1024}],"width":14645,"height":12158,"maxArea":17550000}),
            ),
        ]
    }

    #[test]
    fn rijksmuseum_object_number_reaches_its_iiif_service_and_rights() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-C-5","max_requests":8});
        let result = with_responses(rijks_responses(), || invoke(&request)).unwrap();
        let observation = &result["observation"];
        let roles: Vec<&str> = observation["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["role"].as_str().unwrap())
            .collect();
        assert_eq!(
            roles,
            [
                "search",
                "object-record",
                "visual-item-record",
                "digital-object-record",
                "image-service-info"
            ]
        );
        let object = &observation["object"];
        assert_eq!(
            object["provider_object_uri"],
            "https://id.rijksmuseum.nl/200107928"
        );
        assert!(
            object["identifiers"]
                .as_array()
                .unwrap()
                .contains(&json!({"scheme":"Rijksmuseum object number","value":"SK-C-5"}))
        );
        assert_eq!(
            object["titles"][0],
            json!({"text":"De Nachtwacht","language":"nl","language_uri":NL,"role":"http://vocab.getty.edu/aat/300404670"})
        );
        assert_eq!(object["titles"][1]["role"], "former title");
        assert_eq!(object["makers"][0]["label"], "Rembrandt van Rijn");
        assert_eq!(object["makers"][0]["role"], "painter");
        assert_eq!(
            object["dating"],
            json!({"statement":"1642","begin_year":1642,"end_year":1642})
        );
        assert_eq!(object["dimensions"][0]["value"], "379.5");
        assert_eq!(
            object["credit_lines"][0]["text"],
            "On loan from the City of Amsterdam"
        );
        assert_eq!(
            object["depicted"][0]["authority_uris"],
            json!(["http://www.wikidata.org/entity/Q935192"])
        );
        assert_eq!(
            observation["rights"],
            json!([{"scope":"image","provider_field":"subject_to","value":"https://creativecommons.org/publicdomain/mark/1.0/","statement_uri":"https://creativecommons.org/publicdomain/mark/1.0/","label":"Public Domain","stated_in":"https://data.rijksmuseum.nl/202107928?_profile=la-framed"}])
        );
        let image = &observation["images"][0];
        assert_eq!(image["role"], "primary");
        assert_eq!(image["form"], "image_service");
        let service = &image["image_service"];
        assert_eq!(service["service_id"], "https://iiif.micr.io/PJEZO");
        assert_eq!(service["api_version"], "3");
        assert_eq!(
            (
                service["width_pixels"].as_u64(),
                service["height_pixels"].as_u64()
            ),
            (Some(14645), Some(12158))
        );
        assert_eq!(service["max_area"], 17550000);
        assert_eq!(
            service["tiles"],
            json!([{"width":1024,"height":1024,"scale_factors":[1,2,4,8,16]}])
        );
        let info_request = &observation["requests"][4];
        assert_eq!(
            service["info_fingerprint"]["sha256"],
            info_request["sha256"]
        );
        assert_eq!(
            service["info_fingerprint"]["byte_size"],
            info_request["byte_size"]
        );
        assert_eq!(observation["warnings"], json!([]));
    }

    #[test]
    fn repeated_references_are_read_once() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-C-5","max_requests":5});
        let mut responses = rijks_responses();
        let shown = responses[1].1["shows"][0].clone();
        responses[1].1["shows"] = json!([shown.clone(), shown]);
        let access = responses[3].1["access_point"][0].clone();
        responses[3].1["access_point"] = json!([access.clone(), access]);
        let result = with_responses(responses, || invoke(&request)).unwrap();
        assert_eq!(
            result["observation"]["requests"].as_array().unwrap().len(),
            5
        );
        assert_eq!(result["observation"]["images"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn identifiers_keep_every_digit_the_provider_gives() {
        let long = "5001031011048411411411248";
        assert_eq!(
            rijks_data_url(&format!("https://id.rijksmuseum.nl/{long}")).as_deref(),
            Some("https://data.rijksmuseum.nl/5001031011048411411411248?_profile=la-framed")
        );
        assert!(rijks_data_url("https://id.rijksmuseum.nl/12a").is_none());
    }

    #[test]
    fn single_values_read_like_lists() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-C-5","max_requests":8});
        let mut responses = rijks_responses();
        let record = &mut responses[1].1;
        record["classified_as"][0]["notation"] = json!({"@language":"en","@value":"painting"});
        let span = record["produced_by"]["timespan"].clone();
        let mut earlier = span.clone();
        earlier["begin_of_the_begin"] = json!("1640-01-01T00:00:00Z");
        record["produced_by"]["timespan"] = json!([span, earlier]);
        record["produced_by"] = json!([record["produced_by"].clone()]);
        record["shows"] = record["shows"][0].clone();
        responses[3].1["access_point"] = responses[3].1["access_point"][0].clone();
        let result = with_responses(responses, || invoke(&request)).unwrap();
        let object = &result["observation"]["object"];
        assert_eq!(object["classifications"], json!(["painting"]));
        assert_eq!(
            object["dating"],
            json!({"statement":"1642","begin_year":1640,"end_year":1642})
        );
        assert_eq!(object["makers"][0]["label"], "Rembrandt van Rijn");
        assert_eq!(result["observation"]["images"][0]["form"], "image_service");
    }

    #[test]
    fn rijksmuseum_uri_skips_the_search() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"https://id.rijksmuseum.nl/200107928","max_requests":8});
        let result = with_responses(rijks_responses()[1..].to_vec(), || invoke(&request)).unwrap();
        assert_eq!(
            result["observation"]["requests"][0]["role"],
            "object-record"
        );
    }

    #[test]
    fn rijksmuseum_search_must_name_exactly_one_object() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-A","max_requests":8});
        let none = vec![(
            "https://data.rijksmuseum.nl/search/collection?objectNumber=SK-A",
            json!({"type":"OrderedCollectionPage","orderedItems":[]}),
        )];
        let error = with_responses(none, || invoke(&request)).unwrap_err();
        assert!(error.contains("knows no object"), "{error}");
        let two = vec![(
            "https://data.rijksmuseum.nl/search/collection?objectNumber=SK-A",
            json!({"orderedItems":[{"id":"https://id.rijksmuseum.nl/1","type":"HumanMadeObject"},{"id":"https://id.rijksmuseum.nl/2","type":"HumanMadeObject"}]}),
        )];
        let error = with_responses(two, || invoke(&request)).unwrap_err();
        assert!(error.contains("matches 2 objects"), "{error}");
    }

    #[test]
    fn a_single_partial_match_must_carry_the_exact_number() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-C-5","max_requests":8});
        let mut responses = rijks_responses();
        responses[1].1["identified_by"][0]["content"] = json!("SK-C-50");
        let error = with_responses(responses, || invoke(&request)).unwrap_err();
        assert!(
            error.contains("whose object number is not SK-C-5"),
            "{error}"
        );
    }

    #[test]
    fn a_record_for_another_object_is_refused() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"200107928","max_requests":8});
        let mut responses = rijks_responses();
        responses[1].1["id"] = json!("https://id.rijksmuseum.nl/1");
        let error = with_responses(responses, || invoke(&request)).unwrap_err();
        assert!(error.contains("does not describe"), "{error}");
    }

    #[test]
    fn redirects_and_urls_outside_the_provider_hosts_are_refused() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"met","object":"438417","max_requests":8});
        TEST_FETCH.with(|fetch| {
            *fetch.borrow_mut() = Some(Box::new(|_url: &str| {
                Ok(Response {
                    status: 200,
                    final_url: "https://example.org/objects/438417".into(),
                    media_type: None,
                    body: b"{}".to_vec(),
                })
            }));
        });
        let error = invoke(&request).unwrap_err();
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        assert!(error.contains("redirected outside its hosts"), "{error}");

        let mut observer = Observer {
            provider: &MET,
            callback: false,
            limit: 1024,
            budget: 1,
            requests: Vec::new(),
            warnings: Vec::new(),
        };
        let error = observer
            .json("object-record", "http://collectionapi.metmuseum.org/x")
            .unwrap_err();
        assert!(error.contains("outside its hosts"), "{error}");
    }

    #[test]
    fn the_request_budget_is_stated_and_kept() {
        let mut request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"rijksmuseum","object":"SK-C-5"});
        assert!(invoke(&request).unwrap_err().contains("max_requests"));
        request["max_requests"] = json!(4);
        let error = with_responses(rijks_responses(), || invoke(&request)).unwrap_err();
        assert!(error.contains("more than its 4 requests"), "{error}");
    }

    #[test]
    fn responses_keep_the_profile_record_limit() {
        assert_eq!(record_limit().unwrap(), 16 * 1024 * 1024);
    }

    #[test]
    fn met_object_records_its_files_and_public_domain_flag() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"met","object":"438417","max_requests":8});
        let responses = vec![(
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/438417",
            json!({
                "objectID":438417,"isPublicDomain":true,
                "primaryImage":"https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg",
                "additionalImages":[],
                "accessionNumber":"2000.51","title":"Two Men Contemplating the Moon",
                "constituents":[{"constituentID":162021,"role":"Artist","name":"Caspar David Friedrich","constituentULAN_URL":"http://vocab.getty.edu/page/ulan/500116242","constituentWikidata_URL":"https://www.wikidata.org/wiki/Q104884","gender":""}],
                "objectDate":"ca. 1825–30","objectBeginDate":1825,"objectEndDate":1830,
                "medium":"Oil on canvas","dimensions":"13 3/4 x 17 1/4 in. (34.9 x 43.8 cm)",
                "measurements":[{"elementName":"Overall","elementDescription":null,"elementMeasurements":{"Height":34.9,"Width":43.8}}],
                "creditLine":"Wrightsman Fund, 2000","classification":"Paintings","objectName":"Painting",
                "repository":"Metropolitan Museum of Art, New York, NY",
                "objectURL":"https://www.metmuseum.org/art/collection/search/438417",
                "objectWikidata_URL":"https://www.wikidata.org/wiki/Q17281208",
                "tags":[{"term":"Moon","AAT_URL":"http://vocab.getty.edu/page/aat/300379611","Wikidata_URL":"https://www.wikidata.org/wiki/Q405"}],
                "rightsAndReproduction":""
            }),
        )];
        let result = with_responses(responses, || invoke(&request)).unwrap();
        let observation = &result["observation"];
        let object = &observation["object"];
        assert_eq!(
            object["record_url"],
            "https://www.metmuseum.org/art/collection/search/438417"
        );
        assert_eq!(
            object["makers"][0],
            json!({"label":"Caspar David Friedrich","role":"Artist","provider_uri":null,"authority_uris":["http://vocab.getty.edu/page/ulan/500116242","https://www.wikidata.org/wiki/Q104884"]})
        );
        assert_eq!(
            object["dating"],
            json!({"statement":"ca. 1825–30","begin_year":1825,"end_year":1830})
        );
        assert_eq!(
            object["dimensions"][1],
            json!({"kind":"Overall height","value":"34.9","unit":"cm","unit_uri":null,"statement":null})
        );
        assert_eq!(object["depicted"][0]["label"], "Moon");
        assert_eq!(
            observation["rights"],
            json!([{"scope":"image","provider_field":"isPublicDomain","value":"true","statement_uri":null,"label":null,"stated_in":"https://collectionapi.metmuseum.org/public/collection/v1/objects/438417"}])
        );
        assert_eq!(
            observation["images"],
            json!([{"role":"primary","form":"file","access_url":"https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg","image_service":null,"file_url":"https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg"}])
        );
    }

    #[test]
    fn met_object_must_be_numeric() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"met","object":"2000.51","max_requests":8});
        assert!(invoke(&request).unwrap_err().contains("numeric objectID"));
    }

    #[test]
    fn iiif_image_urls_name_their_service() {
        assert_eq!(
            iiif_service_of("https://iiif.micr.io/PJEZO/full/max/0/default.jpg").as_deref(),
            Some("https://iiif.micr.io/PJEZO")
        );
        assert_eq!(
            iiif_service_of(
                "https://example.org/iiif/2/abc%2Fdef/0,0,512,512/!400,400/90/gray.png"
            )
            .as_deref(),
            Some("https://example.org/iiif/2/abc%2Fdef")
        );
        assert_eq!(
            iiif_service_of("https://example.org/a/pct:10,10,50,50/pct:50/0/default.webp")
                .as_deref(),
            Some("https://example.org/a")
        );
        assert_eq!(
            iiif_service_of(
                "https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg"
            ),
            None
        );
        assert_eq!(
            iiif_service_of("https://iiif.micr.io/full/max/0/default.jpg"),
            None
        );
        assert_eq!(
            iiif_service_of("http://iiif.micr.io/PJEZO/full/max/0/default.jpg"),
            None
        );
        assert_eq!(
            iiif_service_of("https://example.org/a/0,0,512,512,9/max/0/default.jpg"),
            None
        );
    }

    #[test]
    fn iiif_version_2_limits_come_from_its_profile() {
        let info = json!({
            "@context":"http://iiif.io/api/image/2/context.json","@id":"https://example.org/iiif/x","protocol":"http://iiif.io/api/image",
            "width":6000,"height":4000,
            "profile":["http://iiif.io/api/image/2/level2.json",{"formats":["jpg","tif"],"qualities":["default","bitonal"],"maxWidth":3000}],
            "tiles":[{"width":512,"scaleFactors":[1,2,4]}]
        });
        let service = parse_image_service(
            &info,
            "https://example.org/iiif/x/info.json",
            (json!(10), json!("0".repeat(64))),
            "2026-11-08T00:00:00Z",
        )
        .unwrap();
        assert_eq!(service["api_version"], "2");
        assert_eq!(service["profile"], "http://iiif.io/api/image/2/level2.json");
        assert_eq!(service["max_width"], 3000);
        assert_eq!(service["max_area"], Value::Null);
        assert_eq!(
            service["tiles"],
            json!([{"width":512,"height":512,"scale_factors":[1,2,4]}])
        );
        assert_eq!(service["formats"], json!(["jpg", "tif"]));
        let unknown = json!({"@context":"http://example.org/context.json","id":"https://example.org/x","width":1,"height":1,"profile":"level0"});
        assert!(
            parse_image_service(
                &unknown,
                "https://example.org/x/info.json",
                (json!(1), json!("0".repeat(64))),
                "2026-11-08T00:00:00Z"
            )
            .is_err()
        );
    }

    #[test]
    fn years_keep_astronomical_numbering() {
        assert_eq!(year_of("1642-01-01T00:00:00Z"), Some(1642));
        assert_eq!(year_of("-0043-01-01T00:00:00Z"), Some(-43));
        assert_eq!(year_of("+0012-01-01"), Some(12));
        assert_eq!(year_of("unknown"), None);
    }

    #[test]
    fn describe_names_providers_and_writes_nothing() {
        let described = invoke(&json!({"operation":"media.describe"})).unwrap();
        assert_eq!(described["providers"][0]["provider"], "rijksmuseum");
        assert_eq!(described["writes"]["media.observe"], json!([]));
        assert!(invoke(&json!({"operation":"media.plant"})).is_err());
    }

    #[test]
    fn contract_examples_conform() {
        let root = PathBuf::from(repo_root());
        for (example, schema) in [
            (
                "media-provider-observation.v1.rijksmuseum.json",
                "media-provider-observation",
            ),
            (
                "media-provider-observation.v1.met.json",
                "media-provider-observation",
            ),
            (
                "artifact-visual-representation.v2.image-service.json",
                "artifact-visual-representation-v2",
            ),
        ] {
            let raw = std::fs::read(root.join("ToS/contracts/examples").join(example)).unwrap();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            crate::source_registry_acquisition::validate_json(&root, &value, schema)
                .unwrap_or_else(|error| panic!("{example}: {error}"));
        }
        let mut file_form: Value = serde_json::from_slice(
            &std::fs::read(root.join("ToS/source-witnesses/artifacts/egyptian/lisht-south/met-khety-32-1-133/representations/inside-lid-iiif-1200/representation.json")).unwrap(),
        )
        .unwrap();
        file_form["$schema"] = json!(
            "https://tree-of-sophia.local/ToS/contracts/artifact-visual-representation-v2.schema.json"
        );
        file_form["schema_version"] = json!("tos_artifact_visual_representation_v2");
        file_form["representation_form"] = json!("file");
        crate::source_registry_acquisition::validate_json(
            &root,
            &file_form,
            "artifact-visual-representation-v2",
        )
        .expect("a v1 file record restated in v2 conforms");
        file_form["representation_id"] =
            json!(format!("tos.image-service.sha256.{}", "0".repeat(64)));
        assert!(
            crate::source_registry_acquisition::validate_json(
                &root,
                &file_form,
                "artifact-visual-representation-v2"
            )
            .is_err(),
            "a file representation carries no image-service fields"
        );
    }
}
