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
use std::collections::HashMap;
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
    /// Hosts the provider serves its image files from, read only when an
    /// observation asks for the files themselves.
    file_hosts: &'static [&'static str],
}

const RIJKSMUSEUM: Provider = Provider {
    id: "rijksmuseum",
    hosts: &["data.rijksmuseum.nl", "iiif.micr.io"],
    file_hosts: &[],
};
const MET: Provider = Provider {
    id: "met",
    hosts: &["collectionapi.metmuseum.org"],
    file_hosts: &["images.metmuseum.org"],
};
/// The National Gallery of Art, Washington: its records come from its open data
/// release (CC0, GitHub, one commit a day), its images from its IIIF services.
const NGA: Provider = Provider {
    id: "nga",
    hosts: &["api.nga.gov", "api.github.com"],
    file_hosts: &[],
};
/// Wikimedia Commons: a file's own page (its size, digest, licence and structured copyright status) and,
/// on Wikidata, the artwork the file reproduces (its titles, makers, inception, collection and inventory).
const COMMONS: Provider = Provider {
    id: "commons",
    hosts: &["commons.wikimedia.org", "www.wikidata.org"],
    file_hosts: &["upload.wikimedia.org", "thumb.wikimedia.org"],
};
/// The Saint Louis Art Museum: its object pages state each work's facts and rights, and link the original file.
const SLAM: Provider = Provider {
    id: "slam",
    hosts: &["www.slam.org"],
    file_hosts: &["d23gjylftzdlen.cloudfront.net"],
};
/// The Staatliche Museen zu Berlin: their collection pages state each work's facts, the photo credit with its
/// rights statement, and link the original file.
const SMB: Provider = Provider {
    id: "smb",
    hosts: &["search.smb.museum"],
    file_hosts: &["search.smb.museum"],
};
const PROVIDERS: &[&Provider] = &[&RIJKSMUSEUM, &MET, &NGA, &COMMONS, &SLAM, &SMB];

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
        .map(|provider| json!({"provider":provider.id,"hosts":provider.hosts,"file_hosts":provider.file_hosts}))
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
    // A primary image served as files is read whole when asked: the observation then fingerprints the
    // provider's file and its smaller copy, so a stand that keeps them can show it holds the bytes observed.
    let files = match request.get("files") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err("files must be a boolean".into()),
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
        "met" => observe_met(&mut observer, object, files)?,
        "nga" => observe_nga(&mut observer, object, request)?,
        "commons" => observe_commons(&mut observer, object, files)?,
        "slam" => observe_slam(&mut observer, object, files)?,
        "smb" => observe_smb(&mut observer, object, files)?,
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
    let completed_at = utcnow()?;
    let mut observation = json!({
        "$schema":format!("https://tree-of-sophia.local/ToS/contracts/{OBSERVATION_SCHEMA}.schema.json"),
        "schema_version":"tos_media_provider_observation_v1",
        "provider":provider.id,
        "requested_object":object,
        "observed_at":observed_at,
        "completed_at":completed_at,
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
    if !found.datasets.is_empty() {
        observation["datasets"] = Value::Array(found.datasets);
    }
    crate::source_registry_acquisition::validate_json(&root, &observation, OBSERVATION_SCHEMA)?;
    Ok(json!({"observation":observation}))
}

struct Found {
    object: Value,
    rights: Vec<Value>,
    images: Vec<Value>,
    /// Published dataset files the record was read from, each with its commit and digest.
    datasets: Vec<Value>,
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

    /// One provider page, read as UTF-8 text and recorded with its fingerprint.
    fn page(&mut self, role: &str, url: &str) -> Result<(String, String)> {
        let (body, final_url) = self.record(role, url, ACCEPT_PAGE)?;
        let text = String::from_utf8(body)
            .map_err(|_| format!("{} page is not UTF-8 text: {url}", self.provider.id))?;
        Ok((text, final_url))
    }

    /// One provider response, parsed as JSON and recorded with its fingerprint.
    fn json(&mut self, role: &str, url: &str) -> Result<(Value, String)> {
        let (body, final_url) = self.record(role, url, ACCEPT_RECORD)?;
        let value: Value = serde_json::from_slice(&body).map_err(|error| {
            format!("{} response is not JSON: {url}: {error}", self.provider.id)
        })?;
        Ok((value, final_url))
    }

    /// One provider response from the provider's hosts, recorded with its fingerprint.
    fn record(&mut self, role: &str, url: &str, accept: &str) -> Result<(Vec<u8>, String)> {
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
            callback_fetch(url, self.limit, accept)?
        } else {
            fetch(url, self.limit, accept)?
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
        self.requests.push(json!({
            "role":role,
            "url":url,
            "final_url":(!self.callback).then_some(response.final_url.as_str()),
            "http_status":if self.callback { None } else { Some(200) },
            "transport":if self.callback { "operator-supplied-bytes" } else { "https" },
            "media_type":response.media_type,
            "byte_size":response.body.len(),
            "sha256":Digest256::of_bytes(&response.body).to_hex(),
            "started_at":started_at,
            "elapsed_seconds":elapsed
        }));
        Ok((response.body, response.final_url))
    }

    /// One image file of the provider, fingerprinted: its bytes, digest, media type and pixel size.
    fn file(&mut self, role: &str, url: &str) -> Result<Value> {
        self.file_within(role, url)?
            .ok_or_else(|| format!("{} file size is out of bounds: {url}", self.provider.id))
    }

    /// A file's fingerprint, or nothing when the file holds more bytes than one response may.
    fn file_within(&mut self, role: &str, url: &str) -> Result<Option<Value>> {
        if self.requests.len() >= self.budget {
            return Err(format!(
                "the observation needs more than its {} requests; raise max_requests",
                self.budget
            ));
        }
        let file_host = |url: &str| {
            https_host(url).is_some_and(|host| self.provider.file_hosts.contains(&host))
        };
        if !file_host(url) {
            return Err(format!(
                "{} bridge refuses a file outside its file hosts: {url}",
                self.provider.id
            ));
        }
        let started_at = utcnow()?;
        let tick = Instant::now();
        // An image host answers 406 to a request that accepts JSON only.
        let response = if self.callback {
            callback_fetch(url, self.limit, ACCEPT_IMAGE)?
        } else {
            fetch(url, self.limit, ACCEPT_IMAGE)?
        };
        let elapsed = tick.elapsed().as_secs_f64();
        if response.status != 200 {
            return Err(format!(
                "{} answered HTTP {} for {url}",
                self.provider.id, response.status
            ));
        }
        if !file_host(&response.final_url) {
            return Err(format!(
                "{} redirected outside its file hosts: {}",
                self.provider.id, response.final_url
            ));
        }
        if response.body.is_empty() {
            return Err(format!("{} served an empty file: {url}", self.provider.id));
        }
        if response.body.len() > self.limit {
            return Ok(None);
        }
        let (media_type, width, height) = image_header(&response.body).ok_or_else(|| {
            format!(
                "{} file is not a JPEG or PNG image: {url}",
                self.provider.id
            )
        })?;
        let sha256 = Digest256::of_bytes(&response.body).to_hex();
        self.requests.push(json!({
            "role":role,
            "url":url,
            "final_url":(!self.callback).then_some(response.final_url.as_str()),
            "http_status":if self.callback { None } else { Some(200) },
            "transport":if self.callback { "operator-supplied-bytes" } else { "https" },
            "media_type":response.media_type,
            "byte_size":response.body.len(),
            "sha256":sha256,
            "started_at":started_at,
            "elapsed_seconds":elapsed
        }));
        Ok(Some(json!({
            "url":url,
            "final_url":(!self.callback).then_some(response.final_url.as_str()),
            "media_type":media_type,
            "byte_size":response.body.len(),
            "sha256":sha256,
            "width_pixels":width,
            "height_pixels":height
        })))
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
    let mut statements: Vec<String> = Vec::new();
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
            if let Some(name) = preferred_content(items(span, "identified_by")) {
                if !statements.contains(&name) {
                    statements.push(name);
                }
            }
            if let Some(year) = nonempty(span, "begin_of_the_begin").and_then(year_of) {
                begin_year = Some(begin_year.map_or(year, |known| known.min(year)));
            }
            if let Some(year) = nonempty(span, "end_of_the_end").and_then(year_of) {
                end_year = Some(end_year.map_or(year, |known| known.max(year)));
            }
        }
    }
    // A production dated by several timespans names each of them: a print of 1606 issued again in 1613 reads "1606; 1613".
    let statement = (!statements.is_empty()).then(|| statements.join("; "));
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
            if digital["id"] != digital_uri.as_str() || digital["type"] != "DigitalObject" {
                return Err(format!(
                    "Rijksmuseum DigitalObject does not describe {digital_uri}"
                ));
            }
            for access in items(&digital, "access_point") {
                let Some(access_url) = nonempty(access, "id") else {
                    continue;
                };
                if seen_access.contains(&access_url.to_owned()) {
                    continue;
                }
                seen_access.push(access_url.to_owned());
                let mut image = match iiif_service_of(access_url) {
                    Some(service_id) => match observer.image_service(&service_id)? {
                        Some(service) => {
                            json!({"form":"image_service","access_url":access_url,"image_service":service,"file_url":null,"preview_url":null,"file_fingerprint":null,"preview_fingerprint":null})
                        }
                        None => file_image(access_url, None),
                    },
                    None if https_host(access_url).is_some() => file_image(access_url, None),
                    None => {
                        observer.warnings.push(format!(
                            "access point is not an HTTPS image address: {access_url}"
                        ));
                        continue;
                    }
                };
                image["rights_source"] = json!(item_final);
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
        datasets: Vec::new(),
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

fn observe_met(observer: &mut Observer, object: &str, files: bool) -> Result<Found> {
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
        images.push(file_image(&url, None));
    }
    // The Met names a smaller copy of its primary image; it belongs to the first file.
    if let (Some(first), Some(small)) = (
        images.first_mut(),
        nonempty(&record, "primaryImageSmall").filter(|url| https_host(url).is_some()),
    ) {
        first["preview_url"] = Value::String(small.to_owned());
    }
    if files {
        if let Some(first) = images.first_mut() {
            if let Some(preview) = first["preview_url"].as_str().map(str::to_owned) {
                first["preview_fingerprint"] = observer.file("image-preview", &preview)?;
            }
            let file = first["file_url"].as_str().unwrap_or_default().to_owned();
            first["file_fingerprint"] = observer.file("image-file", &file)?;
        }
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
        datasets: Vec::new(),
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
// National Gallery of Art.

const NGA_REPOSITORY: &str = "https://github.com/NationalGalleryOfArt/opendata";
const NGA_RAW: &str = "https://raw.githubusercontent.com/NationalGalleryOfArt/opendata";

/// One object of the National Gallery of Art. Its site serves no records to
/// automated readers; the Gallery publishes them instead as its open data
/// release (CSV, CC0), refreshed daily on GitHub. The operator checks that
/// repository out at one commit and names it; the bridge reads the object's
/// rows from that checkout, records each file it read with the commit, its
/// raw URL and its SHA-256, and observes the image's IIIF service live.
fn observe_nga(observer: &mut Observer, object: &str, request: &Value) -> Result<Found> {
    let root = absolute_directory(request, "dataset_root")?;
    let commit = text(request, "dataset_commit")?;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("dataset_commit must be the 40-hex commit of the open data checkout".into());
    }
    dataset_at_commit(&root, commit)?;
    let release_url =
        format!("https://api.github.com/repos/NationalGalleryOfArt/opendata/commits/{commit}");
    let (release, _) = observer.json("dataset-release", &release_url)?;
    if release["sha"] != commit {
        return Err("the NGA repository did not confirm the requested dataset commit".into());
    }
    let numeric = !object.is_empty() && object.bytes().all(|byte| byte.is_ascii_digit());
    let mut datasets = Vec::new();

    let objects = dataset_rows(&root, commit, "data/objects.csv", &mut datasets, |row| {
        if numeric {
            row.get("objectid") == Some(&object.to_owned())
        } else {
            row.get("accessionnum") == Some(&object.to_owned())
        }
    })?;
    let record = match objects.as_slice() {
        [record] => record.clone(),
        [] => return Err(format!("the open data release names no object {object}")),
        _ => {
            return Err(format!(
                "the open data release names {} objects as {object}; name the objectid",
                objects.len()
            ));
        }
    };
    let id = record.get("objectid").cloned().unwrap_or_default();
    let mut images: Vec<HashMap<String, String>> = dataset_rows(
        &root,
        commit,
        "data/published_images.csv",
        &mut datasets,
        |row| row.get("depictstmsobjectid") == Some(&id),
    )?;
    images.sort_by_key(|row| {
        (
            row.get("viewtype").map(String::as_str) != Some("primary"),
            row.get("sequence")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(u64::MAX),
        )
    });
    let dimensions = dataset_rows(
        &root,
        commit,
        "data/objects_dimensions.csv",
        &mut datasets,
        |row| row.get("objectid") == Some(&id),
    )?;

    let field = |key: &str| {
        record
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    // The release marks italics with underscores: _title_.
    let plain = |value: String| value.replace('_', "");
    let raw = |path: &str| format!("{NGA_RAW}/{commit}/{path}");
    let record_url = format!("https://www.nga.gov/collection/art-object-page.{id}.html");
    let mut identifiers = vec![json!({"scheme":"NGA object ID","value":id})];
    if let Some(number) = field("accessionnum") {
        identifiers.push(json!({"scheme":"NGA accession number","value":number}));
    }
    if let Some(wikidata) = field("wikidataid") {
        identifiers.push(json!({"scheme":"Wikidata","value":format!("https://www.wikidata.org/wiki/{wikidata}")}));
    }
    let year = |key: &str| field(key).and_then(|value| value.parse::<i64>().ok());
    let mut dimension_values = Vec::new();
    if let Some(statement) = field("dimensions") {
        dimension_values.push(
            json!({"kind":null,"value":null,"unit":null,"unit_uri":null,"statement":statement}),
        );
    }
    for row in &dimensions {
        let (Some(element), Some(kind), Some(value)) = (
            row.get("element"),
            row.get("dimensiontype"),
            row.get("dimension"),
        ) else {
            continue;
        };
        let value = value.trim_end_matches('0').trim_end_matches('.').to_owned();
        let unit = match row.get("unitname").map(String::as_str) {
            Some("centimeters") => Some("cm"),
            Some("millimeters") => Some("mm"),
            _ => None,
        };
        let kind = format!("{}{} {kind}", element[..1].to_uppercase(), &element[1..]);
        dimension_values
            .push(json!({"kind":kind,"value":value,"unit":unit,"unit_uri":null,"statement":null}));
    }
    let object_json = json!({
        "provider_object_uri":record_url,
        "record_url":record_url,
        "api_url":raw("data/objects.csv"),
        "identifiers":identifiers,
        "titles":field("title").map(plain).map(|text| vec![json!({"text":text,"language":null,"language_uri":null,"role":null})]).unwrap_or_default(),
        "makers":[],
        "production_statements":field("attribution").map(|text| vec![json!({"text":text,"language":null,"language_uri":null})]).unwrap_or_default(),
        "dating":{"statement":field("displaydate"),"begin_year":year("beginyear"),"end_year":year("endyear")},
        "classifications":field("classification").into_iter().collect::<Vec<_>>(),
        "materials":field("medium").into_iter().collect::<Vec<_>>(),
        "dimensions":dimension_values,
        "credit_lines":field("creditline").map(|text| vec![json!({"text":text,"language":null,"language_uri":null})]).unwrap_or_default(),
        "repository":"National Gallery of Art, Washington",
        "depicted":[],
        "visual_item_uri":null
    });

    let mut rights = Vec::new();
    let mut image_values = Vec::new();
    for row in images.iter().take(1) {
        let Some(service_id) = row
            .get("iiifurl")
            .filter(|url| https_host(url) == Some("api.nga.gov"))
        else {
            observer
                .warnings
                .push("the published image names no HTTPS IIIF service".into());
            continue;
        };
        if let Some(access) = row.get("openaccess") {
            rights.push(json!({"scope":"image","provider_field":"openaccess","value":access,"statement_uri":null,"label":null,"stated_in":raw("data/published_images.csv")}));
        }
        let access_url = format!("{service_id}/full/full/0/default.jpg");
        match observer.image_service(service_id)? {
            Some(service) => image_values.push(json!({"form":"image_service","access_url":access_url,"image_service":service,"file_url":null,"preview_url":null,"file_fingerprint":null,"preview_fingerprint":null})),
            None => image_values.push(file_image(&access_url, None)),
        }
    }
    assign_roles(&mut image_values);
    Ok(Found {
        object: object_json,
        rights,
        images: image_values,
        datasets,
    })
}

// ---------------------------------------------------------------------------
// Wikimedia Commons: the file page and its structured data, the artwork on Wikidata.

const COMMONS_API: &str = "https://commons.wikimedia.org/w/api.php";
const WIKIDATA_API: &str = "https://www.wikidata.org/w/api.php";
/// The width of the smaller copy a stand shows on its cards: one of Wikimedia's standard thumbnail widths.
const COMMONS_PREVIEW_WIDTH: u64 = 960;
/// Wikimedia's larger standard thumbnail widths; its image servers serve these to automated clients,
/// and refuse them originals and other widths.
const COMMONS_FILE_WIDTHS: [u64; 3] = [3840, 1920, 1280];

/// The values of a Wikibase statement list that is not deprecated, preferred ones first.
fn wikibase_statements(claims: &Value, property: &str) -> Vec<Value> {
    let mut statements: Vec<Value> = claims
        .get(property)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|statement| statement["rank"].as_str() != Some("deprecated"))
        .cloned()
        .collect();
    statements.sort_by_key(|statement| statement["rank"].as_str() != Some("preferred"));
    statements
}

fn wikibase_item(statement: &Value) -> Option<String> {
    statement["mainsnak"]["datavalue"]["value"]["id"]
        .as_str()
        .filter(|id| {
            id.starts_with('Q') && id.len() > 1 && id[1..].bytes().all(|byte| byte.is_ascii_digit())
        })
        .map(str::to_owned)
}

fn wikibase_qualifier_items(statement: &Value, property: &str) -> Vec<String> {
    statement["qualifiers"][property]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|snak| snak["datavalue"]["value"]["id"].as_str().map(str::to_owned))
        .collect()
}

/// The year of a Wikibase time value ("+1629-00-00T00:00:00Z") and its precision.
fn wikibase_year(value: &Value) -> Option<(i64, u64)> {
    let time = value["time"].as_str()?;
    let (sign, rest) = match time.as_bytes().first()? {
        b'+' => (1, &time[1..]),
        b'-' => (-1, &time[1..]),
        _ => return None,
    };
    let year: i64 = rest.split('-').next()?.parse().ok()?;
    Some((sign * year, value["precision"].as_u64()?))
}

/// The plain text of an HTML fragment Commons gives in its extended metadata.
fn commons_plain(html: &str) -> String {
    // Templates hide machine-readable copies of a value ("date QS:P571,…") in elements that are not displayed.
    let mut text = String::with_capacity(html.len());
    let mut hidden = 0usize;
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        if hidden == 0 {
            text.push_str(&rest[..start]);
        }
        let Some(stop) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[start + 1..start + stop];
        rest = &rest[start + stop + 1..];
        let name = tag
            .trim_start_matches('/')
            .split(|character: char| character.is_whitespace() || character == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let void = tag.ends_with('/')
            || matches!(
                name.as_str(),
                "br" | "img" | "hr" | "meta" | "link" | "input" | "wbr"
            );
        if tag.starts_with('/') {
            hidden = hidden.saturating_sub(1);
        } else if !void && (hidden > 0 || tag.replace(' ', "").contains("display:none")) {
            hidden += 1;
        } else if hidden == 0 && name == "br" {
            text.push(' ');
        }
    }
    if hidden == 0 {
        text.push_str(rest);
    }
    let text = text
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The years a dating's words name: each number of three or four digits, a decade ("1840s") to its last year.
fn stated_years(text: &str) -> Vec<i64> {
    let bytes = text.as_bytes();
    let mut years = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        if (3..=4).contains(&(at - start)) {
            let year: i64 = text[start..at].parse().unwrap_or_default();
            years.push(year);
            if bytes.get(at) == Some(&b's') && year % 10 == 0 {
                years.push(year + 9);
            }
        }
    }
    years
}

/// An address without its query and fragment: Commons adds tracking parameters to the files it names.
fn without_query(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).to_owned()
}

fn observe_commons(observer: &mut Observer, object: &str, files: bool) -> Result<Found> {
    let name = object.strip_prefix("File:").unwrap_or(object).trim();
    if name.is_empty() || name.contains(['|', '#', '[', ']', '{', '}']) {
        return Err(format!("a Commons file is named by its title: {object}"));
    }
    let title = format!("File:{name}");
    let info_url = format!(
        "{COMMONS_API}?action=query&format=json&formatversion=2&prop=imageinfo&iiprop=size%7Csha1%7Curl%7Cmime%7Cextmetadata&iiurlwidth={COMMONS_PREVIEW_WIDTH}&titles={}",
        percent_encode(&title)
    );
    let (info, _) = observer.json("file-record", &info_url)?;
    let page = info["query"]["pages"]
        .as_array()
        .and_then(|pages| pages.first())
        .ok_or("Commons answered without the file's page")?;
    if page.get("missing").is_some() || page.get("invalid").is_some() {
        return Err(format!("Commons holds no file {title}"));
    }
    let page_id = page["pageid"]
        .as_u64()
        .ok_or("the Commons page has no id")?;
    let file = page["imageinfo"]
        .as_array()
        .and_then(|list| list.first())
        .ok_or("Commons names no image for the file")?;
    let description = nonempty(file, "descriptionurl")
        .filter(|url| https_host(url) == Some("commons.wikimedia.org"))
        .ok_or("the Commons file names no description page")?
        .to_owned();
    let meta = |key: &str| {
        file["extmetadata"][key]["value"]
            .as_str()
            .map(commons_plain)
            .filter(|text| !text.is_empty())
    };

    let entity_url = format!("{COMMONS_API}?action=wbgetentities&format=json&ids=M{page_id}");
    let (entities, _) = observer.json("file-structured-data", &entity_url)?;
    let media = &entities["entities"][format!("M{page_id}")];
    let statements = media.get("statements").cloned().unwrap_or(Value::Null);
    let artwork = wikibase_statements(&statements, "P6243")
        .iter()
        .find_map(wikibase_item)
        .ok_or("the Commons file names no artwork it represents (P6243)")?;
    let status = wikibase_statements(&statements, "P6216").first().cloned();

    let item_url = format!("https://www.wikidata.org/wiki/Special:EntityData/{artwork}.json");
    let (item, item_final) = observer.json("object-record", &item_url)?;
    let entity = item["entities"]
        .as_object()
        .and_then(|entities| entities.values().next())
        .ok_or("Wikidata answered without the artwork")?;
    let claims = &entity["claims"];
    let current = |statement: &&Value| statement["qualifiers"]["P582"].is_null();
    let collections: Vec<String> = wikibase_statements(claims, "P195")
        .iter()
        .filter(current)
        .filter_map(wikibase_item)
        .collect();
    let creators: Vec<String> = wikibase_statements(claims, "P170")
        .iter()
        .filter_map(wikibase_item)
        .collect();
    let materials: Vec<String> = wikibase_statements(claims, "P186")
        .iter()
        .filter_map(wikibase_item)
        .collect();
    let kinds: Vec<String> = wikibase_statements(claims, "P31")
        .iter()
        .filter_map(wikibase_item)
        .collect();
    let mut wanted: Vec<String> = Vec::new();
    for id in collections
        .iter()
        .take(1)
        .chain(&creators)
        .chain(&materials)
        .chain(&kinds)
    {
        if !wanted.contains(id) && wanted.len() < 50 {
            wanted.push(id.clone());
        }
    }
    let labels = if wanted.is_empty() {
        Value::Null
    } else {
        let labels_url = format!(
            "{WIKIDATA_API}?action=wbgetentities&format=json&props=labels&languages=en&ids={}",
            wanted.join("%7C")
        );
        observer.json("authority-labels", &labels_url)?.0
    };
    let label = |id: &str| {
        labels["entities"][id]["labels"]["en"]["value"]
            .as_str()
            .map(str::to_owned)
    };
    let entity_uri = |id: &str| format!("http://www.wikidata.org/entity/{id}");

    let mut identifiers = vec![
        json!({"scheme":"Wikimedia Commons file","value":title}),
        json!({"scheme":"Wikidata","value":entity_uri(&artwork)}),
    ];
    // The inventory number the current collection gives, else the first one stated.
    let numbers = wikibase_statements(claims, "P217");
    let number = numbers
        .iter()
        .find(|statement| {
            collections.first().is_some_and(|collection| {
                wikibase_qualifier_items(statement, "P195").contains(collection)
            })
        })
        .or(numbers.first())
        .and_then(|statement| statement["mainsnak"]["datavalue"]["value"].as_str());
    if let Some(number) = number {
        identifiers.push(json!({"scheme":"Collection accession number","value":number}));
    }
    // One title, as a card names a work: its English title, else its English label, else its title in its own language.
    let stated: Vec<(String, String)> = wikibase_statements(claims, "P1476")
        .iter()
        .filter_map(|statement| {
            let value = &statement["mainsnak"]["datavalue"]["value"];
            Some((
                value["text"].as_str()?.to_owned(),
                value["language"].as_str()?.to_owned(),
            ))
        })
        .collect();
    let english_label = entity["labels"]["en"]["value"]
        .as_str()
        .map(|text| (text.to_owned(), "en".to_owned()));
    let titles: Vec<Value> = stated
        .iter()
        .find(|(_, language)| language == "en")
        .cloned()
        .or(english_label)
        .or_else(|| stated.first().cloned())
        .map(|(text, language)| json!({"text":text,"language":language,"language_uri":null,"role":null}))
        .into_iter()
        .collect();
    let makers: Vec<Value> = creators
        .iter()
        .filter_map(|id| Some(json!({"label":label(id)?,"role":null,"provider_uri":null,"authority_uris":[entity_uri(id)]})))
        .collect();
    // The dating in the words of the file's page, its years from the artwork's inception and its bounds.
    // The years the artwork's inception records: its year or decade, else the bounds its qualifiers state.
    let inception = wikibase_statements(claims, "P571").into_iter().next();
    let recorded = inception.as_ref().and_then(|statement| {
        let (year, precision) = wikibase_year(&statement["mainsnak"]["datavalue"]["value"])?;
        let bound = |properties: [&str; 2]| {
            properties.iter().find_map(|property| {
                statement["qualifiers"][*property]
                    .as_array()
                    .and_then(|list| list.first())
                    .and_then(|snak| wikibase_year(&snak["datavalue"]["value"]))
                    .map(|(year, _)| year)
            })
        };
        let (earliest, latest) = (bound(["P1319", "P580"]), bound(["P1326", "P582"]));
        // A century or a millennium is written in more than one way, so only its bounds name years.
        let (start, end) = match precision {
            9.. => (year, year),
            8 => (year - year.rem_euclid(10), year - year.rem_euclid(10) + 9),
            _ => return Some((earliest?, latest?)),
        };
        Some((earliest.unwrap_or(start), latest.unwrap_or(end)))
    });
    // The page's dating speaks for the work, with the years its words name; a camera's timestamp
    // ("2018-01-14 13:47:24") is the photograph's own.
    let statement = meta("DateTimeOriginal").filter(|text| {
        let bytes = text.as_bytes();
        let clock = bytes.windows(5).any(|window| {
            window[0].is_ascii_digit()
                && window[1].is_ascii_digit()
                && window[2] == b':'
                && window[3].is_ascii_digit()
                && window[4].is_ascii_digit()
        });
        !clock && bytes.iter().any(u8::is_ascii_digit)
    });
    let stated = statement.as_deref().and_then(|text| {
        let years = stated_years(text);
        Some((*years.iter().min()?, *years.iter().max()?))
    });
    let dating = match (statement, stated.or(recorded)) {
        (Some(statement), Some((begin, end))) => {
            json!({"statement":statement,"begin_year":begin,"end_year":end})
        }
        (None, Some((begin, end))) => {
            let statement = if begin == end {
                begin.to_string()
            } else {
                format!("{begin}–{end}")
            };
            json!({"statement":statement,"begin_year":begin,"end_year":end})
        }
        _ => json!({"statement":null,"begin_year":null,"end_year":null}),
    };
    let mut dimensions = Vec::new();
    for (property, kind) in [("P2048", "height"), ("P2049", "width")] {
        let Some(statement) = wikibase_statements(claims, property).into_iter().next() else {
            continue;
        };
        let value = &statement["mainsnak"]["datavalue"]["value"];
        let unit = match value["unit"].as_str() {
            Some("http://www.wikidata.org/entity/Q174728") => "cm",
            Some("http://www.wikidata.org/entity/Q174789") => "mm",
            _ => continue,
        };
        let Some(amount) = value["amount"]
            .as_str()
            .map(|amount| amount.trim_start_matches('+').to_owned())
        else {
            continue;
        };
        dimensions
            .push(json!({"kind":kind,"value":amount,"unit":unit,"unit_uri":null,"statement":null}));
    }
    let depicted: Vec<Value> = wikibase_statements(claims, "P180")
        .iter()
        .filter_map(wikibase_item)
        .map(|id| json!({"label":null,"kind":"depicts","provider_uri":null,"authority_uris":[entity_uri(&id)]}))
        .collect();
    let record_url = format!("https://www.wikidata.org/wiki/{artwork}");
    let object_json = json!({
        "provider_object_uri":description,
        "record_url":record_url,
        "api_url":item_final,
        "identifiers":identifiers,
        "titles":titles,
        "makers":makers,
        "production_statements":[],
        "dating":dating,
        "classifications":kinds.iter().filter_map(|id| label(id)).collect::<Vec<_>>(),
        "materials":materials.iter().filter_map(|id| label(id)).collect::<Vec<_>>(),
        "dimensions":dimensions,
        "credit_lines":[],
        "repository":collections.first().and_then(|id| label(id)),
        "depicted":depicted,
        "visual_item_uri":null
    });

    // The file's copyright status as its structured data states it, with the licence its page names.
    let mut rights = Vec::new();
    let licence_url = meta("LicenseUrl")
        .map(|url| url.replace("http://", "https://"))
        .filter(|url| url.starts_with("https://creativecommons.org/publicdomain/"));
    if let Some(status) = status.as_ref().and_then(wikibase_item) {
        rights.push(json!({"scope":"image","provider_field":"P6216","value":status,"statement_uri":licence_url,"label":meta("LicenseShortName"),"stated_in":description}));
    }
    // The licence the file's page states through its template ("pd" for a public-domain template).
    if let Some(licence) = meta("License") {
        rights.push(json!({"scope":"image","provider_field":"License","value":licence,"statement_uri":licence_url,"label":meta("LicenseShortName"),"stated_in":description}));
    }
    if let Some(copyrighted) = meta("Copyrighted") {
        rights.push(json!({"scope":"image","provider_field":"Copyrighted","value":copyrighted,"statement_uri":null,"label":meta("UsageTerms"),"stated_in":description}));
    }

    // Wikimedia serves automated clients its standard thumbnail widths: the file is the largest one the original
    // holds, the smaller copy for cards its 960-pixel one; an original narrower than both is read itself.
    let width = file["width"].as_u64().unwrap_or(0);
    let preview = nonempty(file, "thumburl")
        .map(without_query)
        .filter(|url| https_host(url).is_some());
    let original = nonempty(file, "url")
        .map(without_query)
        .filter(|url| https_host(url).is_some());
    let step = COMMONS_FILE_WIDTHS.into_iter().find(|step| *step <= width);
    let file_url = match (step, preview.as_ref()) {
        (Some(step), Some(preview)) => {
            let marker = format!("/{COMMONS_PREVIEW_WIDTH}px-");
            preview
                .contains(&marker)
                .then(|| preview.replacen(&marker, &format!("/{step}px-"), 1))
        }
        _ => original.clone(),
    };
    let mut images = Vec::new();
    match file_url {
        Some(url) => {
            if step.is_some() {
                observer.warnings.push(format!(
                    "the file is read as Commons' {}-pixel standard copy of its {width}-pixel original",
                    step.unwrap_or_default()
                ));
            }
            // An original no wider than the card's copy is its own smaller copy.
            let preview = preview.filter(|preview| *preview != url);
            let mut image = file_image(&url, Some(preview.as_deref().unwrap_or(&url)));
            if files {
                if let Some(preview) = preview.as_deref() {
                    image["preview_fingerprint"] = observer.file("image-preview", preview)?;
                }
                image["file_fingerprint"] = observer.file("image-file", &url)?;
                if preview.is_none() {
                    image["preview_fingerprint"] = image["file_fingerprint"].clone();
                }
            }
            images.push(image);
        }
        None => observer
            .warnings
            .push("the Commons file names no image address".into()),
    }
    assign_roles(&mut images);
    Ok(Found {
        object: object_json,
        rights,
        images,
        datasets: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Saint Louis Art Museum: the object page's fields, its rights and its original file.

/// The text of an HTML fragment: its tags dropped, its character references read, its spaces collapsed.
fn html_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut inside = false;
    for character in html.chars() {
        match character {
            '<' => inside = true,
            '>' => {
                inside = false;
                text.push(' ');
            }
            _ if !inside => text.push(character),
            _ => {}
        }
    }
    let text = text
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#8211;", "–")
        .replace("&#8217;", "’")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.replace(" ,", ",")
}

/// The fields of the page's definition lists, as label and text, in the page's order.
fn definition_fields(page: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find("<dt") {
        rest = &rest[start..];
        let Some(open) = rest.find('>') else { break };
        let Some(close) = rest.find("</dt>") else {
            break;
        };
        let label = html_text(&rest[open + 1..close]);
        rest = &rest[close + 5..];
        let Some(value_start) = rest.find("<dd") else {
            break;
        };
        if rest[..value_start].contains("<dt") {
            continue;
        }
        let after = &rest[value_start..];
        let Some(value_open) = after.find('>') else {
            break;
        };
        let Some(value_close) = after.find("</dd>") else {
            break;
        };
        fields.push((label, html_text(&after[value_open + 1..value_close])));
        rest = &after[value_close + 5..];
    }
    fields
}

fn observe_slam(observer: &mut Observer, object: &str, files: bool) -> Result<Found> {
    if !all_digits(object) {
        return Err(format!(
            "a Saint Louis Art Museum object is named by the number of its page: {object}"
        ));
    }
    let record_url = format!("https://www.slam.org/collection/objects/{object}/");
    let (page, page_final) = observer.page("object-record", &record_url)?;
    let field = |label: &str| {
        definition_fields(&page)
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(label))
            .map(|(_, value)| value)
            .filter(|value| !value.is_empty())
    };
    let title = page
        .find("<h1")
        .and_then(|start| {
            let rest = &page[start..];
            let open = rest.find('>')?;
            let close = rest.find("</h1>")?;
            Some(html_text(&rest[open + 1..close]))
        })
        .filter(|title| !title.is_empty())
        .ok_or("the Saint Louis Art Museum page names no title")?;
    let number =
        field("Object Number").ok_or("the Saint Louis Art Museum page names no object number")?;
    // The years its dating states, first and last.
    let statement = field("Date");
    let years: Vec<i64> = statement
        .as_deref()
        .unwrap_or_default()
        .split(|character: char| !character.is_ascii_digit())
        .filter(|digits| (3..=4).contains(&digits.len()))
        .filter_map(|digits| digits.parse().ok())
        .collect();
    let object_json = json!({
        "provider_object_uri":record_url,
        "record_url":record_url,
        "api_url":page_final,
        "identifiers":[{"scheme":"Saint Louis Art Museum object number","value":number}],
        "titles":[{"text":title,"language":"en","language_uri":null,"role":null}],
        "makers":[],
        "production_statements":field("Artist").map(|text| vec![json!({"text":text,"language":"en","language_uri":null})]).unwrap_or_default(),
        "dating":{"statement":statement,"begin_year":years.first(),"end_year":years.last()},
        "classifications":field("Classification").into_iter().collect::<Vec<_>>(),
        "materials":field("Material").into_iter().collect::<Vec<_>>(),
        "dimensions":field("Dimensions").map(|statement| vec![json!({"kind":null,"value":null,"unit":null,"unit_uri":null,"statement":statement})]).unwrap_or_default(),
        "credit_lines":field("Credit Line").map(|text| vec![json!({"text":text,"language":"en","language_uri":null})]).unwrap_or_default(),
        "repository":"Saint Louis Art Museum",
        "depicted":[],
        "visual_item_uri":null
    });
    let rights: Vec<Value> = field("Rights")
        .map(|value| json!({"scope":"image","provider_field":"Rights","value":value,"statement_uri":null,"label":value,"stated_in":page_final}))
        .into_iter()
        .collect();
    // The viewer's main image is the original file; the museum's 1600-pixel copy of it is its smaller copy.
    let original = page
        .find("VIEWER_MAIN_IMG")
        .and_then(|start| {
            let rest = &page[start..];
            let url = &rest[rest.find("url:")? + 4..];
            let url = url.trim_start().trim_start_matches(['\'', '"']);
            let end = url.find(['\'', '"'])?;
            Some(url[..end].to_owned())
        })
        .filter(|url| https_host(url).is_some());
    let mut images = Vec::new();
    if let Some(url) = original {
        let preview = url
            .contains("_o2.")
            .then(|| url.replacen("_o2.", "_o3.", 1));
        let mut image = file_image(&url, preview.as_deref());
        if files {
            if let Some(preview) = preview.as_deref() {
                image["preview_fingerprint"] = observer.file("image-preview", preview)?;
            }
            image["file_fingerprint"] = observer.file("image-file", &url)?;
        }
        images.push(image);
    }
    assign_roles(&mut images);
    Ok(Found {
        object: object_json,
        rights,
        images,
        datasets: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Staatliche Museen zu Berlin: the collection page's fields, its photo credit and rights, its original file.

/// The text of the first element after `marker` in `page`, up to `end`.
fn text_after(page: &str, marker: &str, end: &str) -> Option<String> {
    let start = page.find(marker)? + marker.len();
    let rest = &page[start..];
    let stop = rest.find(end)?;
    Some(html_text(&rest[..stop])).filter(|text| !text.is_empty())
}

fn observe_smb(observer: &mut Observer, object: &str, files: bool) -> Result<Found> {
    let object = object.strip_prefix("obj-").unwrap_or(object);
    if !all_digits(object) {
        return Err(format!("an SMB object is named by its ObjID: {object}"));
    }
    let record_url = format!("https://search.smb.museum/object/obj-{object}");
    let (page, page_final) = observer.page("object-record", &record_url)?;
    let main = &page[page
        .find("<main")
        .ok_or("the SMB page has no main content")?..];
    let title = main
        .find("<h1")
        .and_then(|start| text_after(&main[start..], ">", "</h1>"))
        .ok_or("the SMB page names no title")?;
    let number =
        text_after(main, "Ident. Nr.:", "</p>").ok_or("the SMB page names no inventory number")?;
    let institution = text_after(main, "Einrichtung:</p>", "</div>");
    let fields = definition_fields(main);
    let field = |label: &str| {
        fields
            .iter()
            .find(|(name, _)| name == label)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    };
    let statement = field("Datierung");
    let years: Vec<i64> = statement
        .as_deref()
        .unwrap_or_default()
        .split(|character: char| !character.is_ascii_digit())
        .filter(|digits| (3..=4).contains(&digits.len()))
        .filter_map(|digits| digits.parse().ok())
        .collect();
    // "Höhe x Breite: 180 x 242 cm": the height and width the page states, in its unit.
    let mut dimensions = Vec::new();
    if let Some(sizes) = field("Abmessungen") {
        dimensions
            .push(json!({"kind":null,"value":null,"unit":null,"unit_uri":null,"statement":sizes}));
        if let Some(rest) = sizes.strip_prefix("Höhe x Breite:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let [height, "x", width, unit] = parts.as_slice() {
                let unit = match *unit {
                    "cm" => Some("cm"),
                    "mm" => Some("mm"),
                    _ => None,
                };
                let number = |value: &str| {
                    let value = value.replace(',', ".");
                    value.parse::<f64>().ok().map(|_| value)
                };
                if let (Some(unit), Some(height), Some(width)) =
                    (unit, number(height), number(width))
                {
                    dimensions.push(json!({"kind":"height","value":height,"unit":unit,"unit_uri":null,"statement":null}));
                    dimensions.push(json!({"kind":"width","value":width,"unit":unit,"unit_uri":null,"statement":null}));
                }
            }
        }
    }
    // Each photograph's credit names its maker and ends with its rights statement.
    let pdm = "Public Domain Mark 1.0";
    let mut credits: Vec<String> = Vec::new();
    let mut rest = main;
    while let Some(start) = rest.find("Fotonachweis:") {
        rest = &rest[start + "Fotonachweis:".len()..];
        if let Some(credit) = rest.find("</div>").map(|stop| html_text(&rest[..stop])) {
            if !credit.is_empty() && !credits.contains(&credit) {
                credits.push(credit);
            }
        }
    }
    let rights: Vec<Value> = credits
        .iter()
        .map(|credit| {
            let statement = credit.ends_with(pdm).then_some("https://creativecommons.org/publicdomain/mark/1.0/");
            json!({"scope":"image","provider_field":"Fotonachweis","value":credit,"statement_uri":statement,"label":statement.map(|_| pdm),"stated_in":page_final})
        })
        .collect();
    let repository = institution
        .map(|name| format!("Staatliche Museen zu Berlin, {name}"))
        .unwrap_or_else(|| "Staatliche Museen zu Berlin".into());
    let object_json = json!({
        "provider_object_uri":format!("https://id.smb.museum/object/{object}"),
        "record_url":record_url,
        "api_url":page_final,
        "identifiers":[{"scheme":"SMB ObjID","value":object},{"scheme":"SMB inventory number","value":number}],
        "titles":[{"text":title,"language":"de","language_uri":null,"role":null}],
        "makers":[],
        "production_statements":field("Beteiligte").map(|text| vec![json!({"text":text,"language":"de","language_uri":null})]).unwrap_or_default(),
        "dating":{"statement":statement,"begin_year":years.first(),"end_year":years.last()},
        "classifications":field("Systematik/Art").into_iter().collect::<Vec<_>>(),
        "materials":field("Material / Technik").into_iter().collect::<Vec<_>>(),
        "dimensions":dimensions,
        "credit_lines":[],
        "repository":repository,
        "depicted":[],
        "visual_item_uri":null
    });
    // The gallery: each slide links the original and shows its 1200-pixel copy.
    let attribute = |fragment: &str, name: &str| {
        let start = fragment.find(&format!("{name}=\""))? + name.len() + 2;
        let rest = &fragment[start..];
        Some(rest[..rest.find('"')?].to_owned())
    };
    let absolute = |path: String| {
        if path.starts_with('/') {
            format!("https://search.smb.museum{path}")
        } else {
            path
        }
    };
    let mut images = Vec::new();
    let mut gallery = main
        .find("id=\"gallery-photoswipe\"")
        .map_or("", |start| &main[start..]);
    let marker = "<a href=\"/media/images/originals/";
    while let Some(start) = gallery.find(marker) {
        gallery = &gallery[start..];
        let url = attribute(gallery, "href").map(absolute);
        let preview = gallery
            .find("<img")
            .and_then(|at| attribute(&gallery[at..], "src"))
            .filter(|src| src.starts_with("/media/images/thumbs/"))
            .map(absolute);
        gallery = &gallery[marker.len()..];
        let Some(url) = url else { continue };
        if !files {
            images.push(file_image(&url, preview.as_deref()));
            continue;
        }
        let preview_fingerprint = match preview.as_deref() {
            Some(preview) => observer.file("image-preview", preview)?,
            None => Value::Null,
        };
        // An original larger than one response may hold stays at the museum; its 1200-pixel copy is the file.
        let image = match observer.file_within("image-file", &url)? {
            Some(fingerprint) => {
                let mut image = file_image(&url, preview.as_deref());
                image["file_fingerprint"] = fingerprint;
                image["preview_fingerprint"] = preview_fingerprint;
                image
            }
            None => {
                let Some(copy) = preview.as_deref() else {
                    return Err(format!(
                        "the SMB original exceeds the record limit and has no copy: {url}"
                    ));
                };
                observer.warnings.push(format!(
                    "the original {url} exceeds the record limit; the file is the museum's 1200-pixel copy"
                ));
                let mut image = file_image(copy, Some(copy));
                image["file_fingerprint"] = preview_fingerprint.clone();
                image["preview_fingerprint"] = preview_fingerprint;
                image
            }
        };
        images.push(image);
    }
    assign_roles(&mut images);
    Ok(Found {
        object: object_json,
        rights,
        images,
        datasets: Vec::new(),
    })
}

/// The checkout holds exactly the named commit, its data files as committed.
fn dataset_at_commit(root: &Path, commit: &str) -> Result<()> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| format!("cannot read the open data checkout with git: {error}"))
    };
    let origin = git(&["remote", "get-url", "origin"])?;
    if !origin.status.success()
        || !matches!(
            String::from_utf8_lossy(&origin.stdout).trim(),
            NGA_REPOSITORY | "https://github.com/NationalGalleryOfArt/opendata.git"
        )
    {
        return Err(
            "the dataset checkout must name the NGA open data release as its HTTPS origin".into(),
        );
    }
    let head = git(&["rev-parse", "HEAD"])?;
    if !head.status.success() || String::from_utf8_lossy(&head.stdout).trim() != commit {
        return Err(format!("the open data checkout is not at {commit}"));
    }
    let dirty = git(&["status", "--porcelain", "--", "data"])?;
    if !dirty.status.success() || !dirty.stdout.is_empty() {
        return Err("the open data checkout's data files differ from its commit".into());
    }
    Ok(())
}

/// The rows of one release file that `keep` selects, read as a stream; the file
/// is recorded with its commit, raw URL, size and SHA-256.
fn dataset_rows(
    root: &Path,
    commit: &str,
    path: &str,
    datasets: &mut Vec<Value>,
    keep: impl Fn(&HashMap<String, String>) -> bool,
) -> Result<Vec<HashMap<String, String>>> {
    use std::io::Read;
    let mut blob = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "blob", &format!("{commit}:{path}")])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot read {path} of the open data release: {error}"))?;
    let file = blob.stdout.take().ok_or("missing Git blob stream")?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut digest = tos_foundation::Digest256Hasher::new();
    let mut size = 0usize;
    let mut csv = CsvReader::default();
    let mut header: Option<Vec<String>> = None;
    let mut kept = Vec::new();
    let mut buffer = vec![0u8; 1 << 20];
    let parsed = (|| -> Result<()> {
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| format!("cannot read {path}: {error}"))?;
            let chunk = &buffer[..read];
            digest.update(chunk);
            size += read;
            for record in csv.feed(chunk, read == 0)? {
                match &header {
                    None => header = Some(record),
                    Some(names) => {
                        if record.len() != names.len() {
                            return Err(format!(
                                "{path}: a row holds {} fields where the header names {}",
                                record.len(),
                                names.len()
                            ));
                        }
                        let row: HashMap<String, String> =
                            names.iter().cloned().zip(record).collect();
                        if keep(&row) {
                            kept.push(row);
                        }
                    }
                }
            }
            if read == 0 {
                break;
            }
        }
        Ok(())
    })();
    drop(reader);
    if !blob.wait().map_err(|error| error.to_string())?.success() && parsed.is_ok() {
        return Err(format!("cannot read committed dataset blob {path}"));
    }
    parsed?;
    datasets.push(json!({
        "repository":NGA_REPOSITORY,
        "commit":commit,
        "path":path,
        "url":format!("{NGA_RAW}/{commit}/{path}"),
        "byte_size":size,
        "sha256":digest.finalize().to_hex()
    }));
    Ok(kept)
}

/// A streaming reader of RFC 4180 CSV: quoted fields may hold commas, doubled
/// quotes and line breaks; records end at CRLF or LF.
#[derive(Default)]
struct CsvReader {
    field: Vec<u8>,
    record: Vec<String>,
    quoted: bool,
    quote_pending: bool,
    started: bool,
}

impl CsvReader {
    fn feed(&mut self, chunk: &[u8], end: bool) -> Result<Vec<Vec<String>>> {
        let mut records = Vec::new();
        for &byte in chunk {
            if self.quoted {
                if self.quote_pending {
                    self.quote_pending = false;
                    if byte == b'"' {
                        self.field.push(b'"');
                        continue;
                    }
                    self.quoted = false;
                } else if byte == b'"' {
                    self.quote_pending = true;
                    continue;
                } else {
                    self.field.push(byte);
                    continue;
                }
            }
            match byte {
                b'"' if self.field.is_empty() => {
                    self.quoted = true;
                    self.started = true;
                }
                b',' => {
                    self.push_field()?;
                    self.started = true;
                }
                b'\r' => {}
                b'\n' => {
                    if self.started || !self.field.is_empty() || !self.record.is_empty() {
                        self.push_field()?;
                        records.push(std::mem::take(&mut self.record));
                    }
                    self.started = false;
                }
                byte => {
                    self.field.push(byte);
                    self.started = true;
                }
            }
        }
        if end {
            if self.quoted && !self.quote_pending {
                return Err("a CSV field opens a quote that never closes".into());
            }
            self.quoted = false;
            self.quote_pending = false;
            if self.started || !self.field.is_empty() || !self.record.is_empty() {
                self.push_field()?;
                records.push(std::mem::take(&mut self.record));
            }
        }
        Ok(records)
    }

    fn push_field(&mut self) -> Result<()> {
        let field = String::from_utf8(std::mem::take(&mut self.field))
            .map_err(|_| "a CSV field is not UTF-8".to_owned())?;
        self.record.push(field);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transport.

const ACCEPT_RECORD: &str = "application/ld+json, application/json;q=0.9";
const ACCEPT_IMAGE: &str = "image/jpeg, image/png;q=0.9, image/*;q=0.8";
const ACCEPT_PAGE: &str = "text/html, application/xhtml+xml;q=0.9";

fn fetch(url: &str, limit: usize, accept: &str) -> Result<Response> {
    #[cfg(test)]
    if let Some(result) = TEST_FETCH.with(|fetch| fetch.borrow().as_ref().map(|fetch| fetch(url))) {
        return result;
    }
    const MARKER: &[u8] = b"\n__TOS_MEDIA_META__";
    let write_out = "\n__TOS_MEDIA_META__%{http_code}\n%{url_effective}\n%{content_type}";
    let accept = format!("Accept: {accept}");
    let args = [
        "-q",
        "--fail",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--max-time",
        "45",
        "--user-agent",
        "Tree-of-Sophia-media-bridge/1 (https://treeofsophia.com)",
        "--header",
        &accept,
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
fn callback_fetch(url: &str, limit: usize, accept: &str) -> Result<Response> {
    let body = crate::source_acquisition_batch::fetch_request(
        &json!({"url":url,"max_bytes":limit,"accept":accept}),
    )?;
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

/// An image the provider serves as a file, its smaller copy and fingerprints not yet read.
fn file_image(url: &str, preview: Option<&str>) -> Value {
    json!({"form":"file","access_url":url,"image_service":null,"file_url":url,"preview_url":preview,"file_fingerprint":null,"preview_fingerprint":null})
}

/// The media type and pixel size a JPEG or PNG file states in its own header.
fn image_header(bytes: &[u8]) -> Option<(&'static str, u64, u64)> {
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && &bytes[12..16] == b"IHDR" {
        let read = |at: usize| {
            u64::from(u32::from_be_bytes([
                bytes[at],
                bytes[at + 1],
                bytes[at + 2],
                bytes[at + 3],
            ]))
        };
        let (width, height) = (read(16), read(20));
        return (width > 0 && height > 0).then_some(("image/png", width, height));
    }
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    // Walk the JPEG segments to the first start-of-frame, which states the image's height and width.
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xFF {
            at += 1;
            continue;
        }
        if matches!(marker, 0x01 | 0xD0..=0xD7) {
            at += 2;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        let frame = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if frame {
            if at + 9 > bytes.len() || length < 7 {
                return None;
            }
            let height = u64::from(u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]));
            let width = u64::from(u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]));
            return (width > 0 && height > 0).then_some(("image/jpeg", width, height));
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
    None
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
        let mut later = span.clone();
        later["begin_of_the_begin"] = json!("1645-01-01T00:00:00Z");
        later["end_of_the_end"] = json!("1645-12-31T23:59:59Z");
        later["identified_by"][0]["content"] = json!("1645");
        record["produced_by"]["timespan"] = json!([span, earlier, later]);
        record["produced_by"] = json!([record["produced_by"].clone()]);
        record["shows"] = record["shows"][0].clone();
        responses[3].1["access_point"] = responses[3].1["access_point"][0].clone();
        let result = with_responses(responses, || invoke(&request)).unwrap();
        let object = &result["observation"]["object"];
        assert_eq!(object["classifications"], json!(["painting"]));
        assert_eq!(
            object["dating"],
            json!({"statement":"1642; 1645","begin_year":1640,"end_year":1645})
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
            json!([{"role":"primary","form":"file","access_url":"https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg","image_service":null,"file_url":"https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg","preview_url":null,"file_fingerprint":null,"preview_fingerprint":null}])
        );
    }

    /// A JPEG reduced to the segments its header walk reads: an APP0 segment, then a baseline frame.
    fn tiny_jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        bytes.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1, 0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn image_headers_state_type_and_pixel_size() {
        assert_eq!(
            image_header(&tiny_jpeg(4000, 3237)),
            Some(("image/jpeg", 4000, 3237))
        );
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&1759u32.to_be_bytes());
        png.extend_from_slice(&2160u32.to_be_bytes());
        assert_eq!(image_header(&png), Some(("image/png", 1759, 2160)));
        assert_eq!(image_header(b"<html>not an image</html>"), None);
        assert_eq!(image_header(&[0xFF, 0xD8, 0xFF, 0xD9]), None);
    }

    #[test]
    fn met_files_are_read_and_fingerprinted_when_asked() {
        const RECORD: &str =
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/438417";
        const FILE: &str =
            "https://images.metmuseum.org/CRDImages/ep/original/DP-31997-001-NEW.jpg";
        const SMALL: &str =
            "https://images.metmuseum.org/CRDImages/ep/web-large/DP-31997-001-NEW.jpg";
        let record = json!({
            "objectID":438417,"isPublicDomain":true,"primaryImage":FILE,"primaryImageSmall":SMALL,
            "additionalImages":[],"accessionNumber":"2000.51","title":"Two Men Contemplating the Moon",
            "objectURL":"https://www.metmuseum.org/art/collection/search/438417"
        });
        let (file, small) = (tiny_jpeg(4000, 3237), tiny_jpeg(599, 485));
        let serve = move |record: Value, file: Vec<u8>, small: Vec<u8>| {
            TEST_FETCH.with(|fetch| {
                *fetch.borrow_mut() = Some(Box::new(move |url: &str| {
                    let body = match url {
                        RECORD => serde_json::to_vec(&record).unwrap(),
                        FILE => file.clone(),
                        SMALL => small.clone(),
                        _ => return Err(format!("unexpected fixture request: {url}")),
                    };
                    Ok(Response {
                        status: 200,
                        final_url: url.to_owned(),
                        media_type: Some("image/jpeg".into()),
                        body,
                    })
                }));
            });
        };
        let mut request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"met","object":"438417","max_requests":8});

        // Without `files`, the observation names the smaller copy and reads neither file.
        serve(record.clone(), file.clone(), small.clone());
        let light = invoke(&request);
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        let light = light.unwrap();
        let image = &light["observation"]["images"][0];
        assert_eq!(image["preview_url"], SMALL);
        assert_eq!(image["file_fingerprint"], Value::Null);
        assert_eq!(
            light["observation"]["requests"].as_array().unwrap().len(),
            1
        );

        request["files"] = json!(true);
        serve(record.clone(), file.clone(), small.clone());
        let full = invoke(&request);
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        let full = full.unwrap();
        let image = &full["observation"]["images"][0];
        assert_eq!(
            image["file_fingerprint"],
            json!({"url":FILE,"final_url":FILE,"media_type":"image/jpeg","byte_size":file.len(),"sha256":Digest256::of_bytes(&file).to_hex(),"width_pixels":4000,"height_pixels":3237})
        );
        assert_eq!(image["preview_fingerprint"]["width_pixels"], 599);
        let roles: Vec<&str> = full["observation"]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["object-record", "image-preview", "image-file"]);

        // A file the provider names on another host, or bytes that are no image, are refused.
        let mut elsewhere = record.clone();
        elsewhere["primaryImageSmall"] = json!("https://example.org/small.jpg");
        serve(elsewhere, file.clone(), small.clone());
        let error = invoke(&request);
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        assert!(error.unwrap_err().contains("outside its file hosts"));
        serve(record, b"<html></html>".to_vec(), small);
        let error = invoke(&request);
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        assert!(error.unwrap_err().contains("not a JPEG or PNG image"));
    }

    #[test]
    fn csv_records_keep_quotes_commas_and_line_breaks_across_chunks() {
        let text = "a,b,c\r\n1,\"x, \"\"y\"\"\",\"line\r\nbreak\"\r\n2,,last";
        for size in [1, 2, 3, 7, text.len()] {
            let mut reader = CsvReader::default();
            let mut records = Vec::new();
            for chunk in text.as_bytes().chunks(size) {
                records.extend(reader.feed(chunk, false).unwrap());
            }
            records.extend(reader.feed(&[], true).unwrap());
            assert_eq!(
                records,
                vec![
                    vec!["a", "b", "c"],
                    vec!["1", "x, \"y\"", "line\r\nbreak"],
                    vec!["2", "", "last"]
                ],
                "chunk size {size}"
            );
        }
        assert!(CsvReader::default().feed(b"\"open", true).is_err());
    }

    /// A checkout of the open data release reduced to the rows of two objects, committed.
    fn nga_release() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir(&data).unwrap();
        std::fs::write(data.join("objects.csv"), "objectid,accessionnum,title,displaydate,beginyear,endyear,medium,dimensions,attribution,creditline,classification,wikidataid\r\n1138,1942.9.1,The Feast of the Gods,1514/1529,1514,1529,oil on canvas,\"overall: 170.2 x 188 cm (67 x 74 in.)\r\nframed: 203.8 x 218.4 x 7.6 cm\",Giovanni Bellini and Titian,Widener Collection,Painting,Q15008032\r\n46573,1963.10.98,_Woman with a Sunflower_,c. 1905,1905,1905,oil on canvas,,Mary Cassatt,Chester Dale Collection,Painting,\r\n").unwrap();
        std::fs::write(data.join("published_images.csv"), "uuid,iiifurl,viewtype,sequence,openaccess,depictstmsobjectid\r\nb,https://api.nga.gov/iiif/b,alternate,1,1,1138\r\na,https://api.nga.gov/iiif/a,primary,0,1,1138\r\n").unwrap();
        std::fs::write(data.join("objects_dimensions.csv"), "objectid,element,dimensiontype,dimension,unitname\r\n1138,overall,height,170.2000000000,centimeters\r\n1138,overall,width,188.0000000000,centimeters\r\n").unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["-c", "user.name=t", "-c", "user.email=t@example.org"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                status.status.success(),
                "{}",
                String::from_utf8_lossy(&status.stderr)
            );
            String::from_utf8(status.stdout).unwrap()
        };
        git(&["init", "--quiet"]);
        git(&["remote", "add", "origin", NGA_REPOSITORY]);
        git(&["add", "data"]);
        git(&["commit", "--quiet", "-m", "release"]);
        let commit = git(&["rev-parse", "HEAD"]).trim().to_owned();
        (dir, commit)
    }

    #[test]
    fn nga_objects_come_from_the_release_at_its_commit_and_images_from_iiif() {
        let (release, commit) = nga_release();
        let info = json!({"@context":"http://iiif.io/api/image/2/context.json","@id":"https://api.nga.gov/iiif/a","protocol":"http://iiif.io/api/image","width":26900,"height":24642,"tiles":[{"width":256,"height":256,"scaleFactors":[1,2,4]}],"profile":["http://iiif.io/api/image/2/level1.json",{"formats":["jpg"],"qualities":["native","color","gray"]}]});
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"nga","object":"1942.9.1","max_requests":2,"dataset_root":release.path(),"dataset_commit":commit});
        let release_url =
            format!("https://api.github.com/repos/NationalGalleryOfArt/opendata/commits/{commit}");
        let release_response = json!({"sha":commit});
        let result = with_responses(
            vec![
                (&release_url, release_response.clone()),
                ("https://api.nga.gov/iiif/a/info.json", info.clone()),
            ],
            || invoke(&request),
        )
        .unwrap();
        let observation = &result["observation"];
        let object = &observation["object"];
        assert_eq!(
            object["record_url"],
            "https://www.nga.gov/collection/art-object-page.1138.html"
        );
        assert_eq!(object["titles"][0]["text"], "The Feast of the Gods");
        assert_eq!(
            object["production_statements"][0]["text"],
            "Giovanni Bellini and Titian"
        );
        assert_eq!(
            object["dating"],
            json!({"statement":"1514/1529","begin_year":1514,"end_year":1529})
        );
        assert_eq!(
            object["dimensions"][1],
            json!({"kind":"Overall height","value":"170.2","unit":"cm","unit_uri":null,"statement":null})
        );
        assert_eq!(
            object["identifiers"][2]["value"],
            "https://www.wikidata.org/wiki/Q15008032"
        );
        // The primary view is the image, before an alternate listed first.
        assert_eq!(
            observation["images"][0]["image_service"]["service_id"],
            "https://api.nga.gov/iiif/a"
        );
        assert_eq!(
            observation["images"][0]["image_service"]["api_version"],
            "2"
        );
        assert_eq!(
            observation["rights"],
            json!([{"scope":"image","provider_field":"openaccess","value":"1","statement_uri":null,"label":null,"stated_in":format!("{NGA_RAW}/{commit}/data/published_images.csv")}])
        );
        let datasets = observation["datasets"].as_array().unwrap();
        assert_eq!(datasets.len(), 3);
        let objects = std::fs::read(release.path().join("data/objects.csv")).unwrap();
        assert_eq!(
            datasets[0],
            json!({"repository":NGA_REPOSITORY,"commit":commit,"path":"data/objects.csv","url":format!("{NGA_RAW}/{commit}/data/objects.csv"),"byte_size":objects.len(),"sha256":Digest256::of_bytes(&objects).to_hex()})
        );

        // Italic marks come off a title; an object is found by its objectid too.
        let mut by_id = request.clone();
        by_id["object"] = json!("46573");
        let cassatt = with_responses(vec![(&release_url, release_response.clone())], || {
            invoke(&by_id)
        })
        .unwrap();
        assert_eq!(
            cassatt["observation"]["object"]["titles"][0]["text"],
            "Woman with a Sunflower"
        );
        assert!(
            cassatt["observation"]["images"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        // An unknown object is named as such; the release must be the named commit, its data unchanged.
        let mut unknown = by_id.clone();
        unknown["object"] = json!("1");
        assert!(
            with_responses(vec![(&release_url, release_response)], || invoke(&unknown))
                .unwrap_err()
                .contains("names no object 1")
        );
        let mut elsewhere = request.clone();
        elsewhere["dataset_commit"] = json!("0".repeat(40));
        assert!(invoke(&elsewhere).unwrap_err().contains("not at"));
        std::fs::write(
            release.path().join("data/objects_dimensions.csv"),
            "objectid\r\n",
        )
        .unwrap();
        assert!(
            invoke(&request)
                .unwrap_err()
                .contains("differ from its commit")
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

    /// Answers each named URL with its bytes; any other request fails the fixture.
    fn observe_with(request: &Value, table: Vec<(String, Vec<u8>)>) -> Result<Value> {
        let table: HashMap<String, Vec<u8>> = table.into_iter().collect();
        TEST_FETCH.with(|fetch| {
            *fetch.borrow_mut() = Some(Box::new(move |url: &str| {
                table
                    .get(url)
                    .map(|body| Response {
                        status: 200,
                        final_url: url.to_owned(),
                        media_type: None,
                        body: body.clone(),
                    })
                    .ok_or_else(|| format!("unexpected fixture request: {url}"))
            }));
        });
        let result = invoke(request);
        TEST_FETCH.with(|fetch| *fetch.borrow_mut() = None);
        result
    }

    fn roles(result: &Value) -> Vec<String> {
        result["observation"]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["role"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The Laughing Man as Commons and Wikidata serve it, reduced to the fields the bridge reads;
    /// structure and values follow the live records of 2026-11-09.
    fn commons_table(status: &str, artwork: Option<&str>) -> Vec<(String, Vec<u8>)> {
        const THUMBS: &str = "https://thumb.wikimedia.org/wikipedia/commons/thumb/3/3b/Rembrandt_-_The_laughing_man.jpg";
        let info_url = format!(
            "{COMMONS_API}?action=query&format=json&formatversion=2&prop=imageinfo&iiprop=size%7Csha1%7Curl%7Cmime%7Cextmetadata&iiurlwidth=960&titles={}",
            percent_encode("File:Rembrandt - The laughing man.jpg")
        );
        let info = json!({"query":{"pages":[{"pageid":101214140,"title":"File:Rembrandt - The laughing man.jpg","imageinfo":[{
            "width":4816,"height":6000,"mime":"image/jpeg",
            "url":"https://upload.wikimedia.org/wikipedia/commons/3/3b/Rembrandt_-_The_laughing_man.jpg",
            "descriptionurl":"https://commons.wikimedia.org/wiki/File:Rembrandt_-_The_laughing_man.jpg",
            "thumburl":format!("{THUMBS}/960px-Rembrandt_-_The_laughing_man.jpg?utm_source=commons"),
            "extmetadata":{
                "DateTimeOriginal":{"value":"circa 1629–1630"},
                "License":{"value":"pd"},
                "LicenseShortName":{"value":"Public domain"},
                "Copyrighted":{"value":"False"},
                "UsageTerms":{"value":"Public domain"}
            }
        }]}]}});
        let mut media = json!({"statements":{"P6216":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"id":status}}}}]}});
        if let Some(artwork) = artwork {
            media["statements"]["P6243"] =
                json!([{"rank":"normal","mainsnak":{"datavalue":{"value":{"id":artwork}}}}]);
        }
        let item = |id: &str| json!({"rank":"normal","mainsnak":{"datavalue":{"value":{"id":id}}}});
        let entity = json!({"entities":{"Q17275997":{
            "labels":{"en":{"value":"The Laughing Man"}},
            "claims":{
                "P31":[item("Q3305213")],
                "P170":[item("Q5598")],
                "P195":[
                    {"rank":"normal","mainsnak":{"datavalue":{"value":{"id":"Q1471477"}}},"qualifiers":{"P582":[{"datavalue":{"value":{"time":"+1800-00-00T00:00:00Z","precision":9}}}]}},
                    item("Q221092")
                ],
                "P217":[
                    {"rank":"normal","mainsnak":{"datavalue":{"value":"98"}},"qualifiers":{"P195":[{"datavalue":{"value":{"id":"Q1471477"}}}]}},
                    {"rank":"normal","mainsnak":{"datavalue":{"value":"598"}},"qualifiers":{"P195":[{"datavalue":{"value":{"id":"Q221092"}}}]}}
                ],
                "P571":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"time":"+1629-00-00T00:00:00Z","precision":9}}},"qualifiers":{"P1326":[{"datavalue":{"value":{"time":"+1630-00-00T00:00:00Z","precision":9}}}]}}],
                "P2048":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"amount":"+15.4","unit":"http://www.wikidata.org/entity/Q174728"}}}}],
                "P2049":[{"rank":"normal","mainsnak":{"datavalue":{"value":{"amount":"+12.2","unit":"http://www.wikidata.org/entity/Q174728"}}}}]
            }
        }}});
        let labels = json!({"entities":{
            "Q221092":{"labels":{"en":{"value":"Mauritshuis"}}},
            "Q5598":{"labels":{"en":{"value":"Rembrandt"}}},
            "Q3305213":{"labels":{"en":{"value":"painting"}}}
        }});
        vec![
            (info_url, serde_json::to_vec(&info).unwrap()),
            (
                format!("{COMMONS_API}?action=wbgetentities&format=json&ids=M101214140"),
                serde_json::to_vec(&json!({"entities":{"M101214140":media}})).unwrap(),
            ),
            (
                "https://www.wikidata.org/wiki/Special:EntityData/Q17275997.json".into(),
                serde_json::to_vec(&entity).unwrap(),
            ),
            (
                format!(
                    "{WIKIDATA_API}?action=wbgetentities&format=json&props=labels&languages=en&ids=Q221092%7CQ5598%7CQ3305213"
                ),
                serde_json::to_vec(&labels).unwrap(),
            ),
            (
                format!("{THUMBS}/960px-Rembrandt_-_The_laughing_man.jpg"),
                tiny_jpeg(960, 1196),
            ),
            (
                format!("{THUMBS}/3840px-Rembrandt_-_The_laughing_man.jpg"),
                tiny_jpeg(3840, 4784),
            ),
        ]
    }

    #[test]
    fn commons_file_reaches_its_artwork_status_and_standard_copy() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"commons","object":"File:Rembrandt - The laughing man.jpg","max_requests":8,"files":true});
        let result = observe_with(&request, commons_table("Q19652", Some("Q17275997"))).unwrap();
        let observation = &result["observation"];
        let object = &observation["object"];
        assert_eq!(
            object["identifiers"][1]["value"],
            "http://www.wikidata.org/entity/Q17275997"
        );
        // The number the current collection gives, not the one of a former owner.
        assert_eq!(
            object["identifiers"][2],
            json!({"scheme":"Collection accession number","value":"598"})
        );
        assert_eq!(
            object["titles"],
            json!([{"text":"The Laughing Man","language":"en","language_uri":null,"role":null}])
        );
        assert_eq!(object["repository"], "Mauritshuis");
        assert_eq!(object["makers"][0]["label"], "Rembrandt");
        assert_eq!(
            object["dating"],
            json!({"statement":"circa 1629–1630","begin_year":1629,"end_year":1630})
        );
        assert_eq!(object["dimensions"][0]["value"], "15.4");
        assert_eq!(object["classifications"], json!(["painting"]));
        assert_eq!(
            object["record_url"],
            "https://www.wikidata.org/wiki/Q17275997"
        );
        assert_eq!(observation["rights"][0]["provider_field"], "P6216");
        assert_eq!(observation["rights"][0]["value"], "Q19652");
        assert_eq!(observation["rights"][1]["provider_field"], "License");
        assert_eq!(observation["rights"][1]["value"], "pd");
        assert_eq!(observation["rights"][2]["provider_field"], "Copyrighted");
        assert_eq!(
            observation["rights"][0]["stated_in"],
            "https://commons.wikimedia.org/wiki/File:Rembrandt_-_The_laughing_man.jpg"
        );
        let image = &observation["images"][0];
        assert!(image["file_url"].as_str().unwrap().contains("/3840px-"));
        assert!(
            image["preview_url"]
                .as_str()
                .unwrap()
                .ends_with("/960px-Rembrandt_-_The_laughing_man.jpg")
        );
        assert_eq!(image["file_fingerprint"]["width_pixels"], 3840);
        assert_eq!(
            roles(&result),
            [
                "file-record",
                "file-structured-data",
                "object-record",
                "authority-labels",
                "image-preview",
                "image-file"
            ]
        );
        assert!(
            observation["warnings"][0]
                .as_str()
                .unwrap()
                .contains("3840-pixel standard copy of its 4816-pixel original")
        );
    }

    #[test]
    fn commons_text_leaves_out_what_its_templates_hide() {
        assert_eq!(
            commons_plain(
                r#"<time class="dtstart" datetime="1883">1883</time><div style="display: none;">date QS:P571,+1883-00-00T00:00:00Z/9</div>"#
            ),
            "1883"
        );
        assert_eq!(
            commons_plain(
                r#"<span title="circa">circa</span> 1629<br/>–1630<span style="display:none"><b>x</b> QS</span> &amp; after"#
            ),
            "circa 1629 –1630 & after"
        );
    }

    #[test]
    fn a_commons_dating_comes_from_the_artwork_not_the_photograph() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"commons","object":"File:Rembrandt - The laughing man.jpg","max_requests":8});
        let dated = |text: &str, inception: Value| {
            let mut table = commons_table("Q19652", Some("Q17275997"));
            let mut info: Value = serde_json::from_slice(&table[0].1).unwrap();
            info["query"]["pages"][0]["imageinfo"][0]["extmetadata"]["DateTimeOriginal"]["value"] =
                json!(text);
            table[0].1 = serde_json::to_vec(&info).unwrap();
            let mut entity: Value = serde_json::from_slice(&table[2].1).unwrap();
            entity["entities"]["Q17275997"]["claims"]["P571"] = json!([inception]);
            table[2].1 = serde_json::to_vec(&entity).unwrap();
            observe_with(&request, table).unwrap()["observation"]["object"]["dating"].clone()
        };
        let year = |time: &str, precision: u64| json!({"rank":"normal","mainsnak":{"datavalue":{"value":{"time":time,"precision":precision}}}});
        // A photograph's own date is no dating of the painting: the inception's single year speaks.
        // The page's words name the years where they name any.
        assert_eq!(
            dated("circa 1817", year("+1818-00-00T00:00:00Z", 9)),
            json!({"statement":"circa 1817","begin_year":1817,"end_year":1817})
        );
        assert_eq!(
            dated("Unknown date; c. 1840s.", year("+1840-00-00T00:00:00Z", 8)),
            json!({"statement":"Unknown date; c. 1840s.","begin_year":1840,"end_year":1849})
        );
        assert_eq!(
            dated("2018-01-14 13:47:24", year("+1885-00-00T00:00:00Z", 9)),
            json!({"statement":"1885","begin_year":1885,"end_year":1885})
        );
        assert_eq!(
            dated("1880s", year("+1880-00-00T00:00:00Z", 8)),
            json!({"statement":"1880s","begin_year":1880,"end_year":1889})
        );
        // A century without its bounds leaves a gap.
        assert_eq!(
            dated("20th century", year("+2000-00-00T00:00:00Z", 7)),
            json!({"statement":null,"begin_year":null,"end_year":null})
        );
        let mut bounded = year("+2000-00-00T00:00:00Z", 7);
        bounded["qualifiers"] = json!({"P1319":[{"datavalue":{"value":{"time":"+1899-00-00T00:00:00Z","precision":9}}}],"P1326":[{"datavalue":{"value":{"time":"+1900-00-00T00:00:00Z","precision":9}}}]});
        assert_eq!(
            dated("1899–1900", bounded.clone()),
            json!({"statement":"1899–1900","begin_year":1899,"end_year":1900})
        );
        // Without words on the page, the years the inception records, its start and end times among its bounds.
        let mut decade = year("+1885-01-01T00:00:00Z", 8);
        decade["qualifiers"] = json!({"P580":[{"datavalue":{"value":{"time":"+1885-00-00T00:00:00Z","precision":9}}}],"P582":[{"datavalue":{"value":{"time":"+1886-00-00T00:00:00Z","precision":9}}}]});
        assert_eq!(
            dated("2018-01-14 13:47:24", decade),
            json!({"statement":"1885–1886","begin_year":1885,"end_year":1886})
        );
    }

    #[test]
    fn a_small_commons_original_is_its_own_smaller_copy() {
        const ORIGINAL: &str =
            "https://upload.wikimedia.org/wikipedia/commons/3/3b/Rembrandt_-_The_laughing_man.jpg";
        let mut table = commons_table("Q19652", Some("Q17275997"));
        let mut info: Value = serde_json::from_slice(&table[0].1).unwrap();
        let file = &mut info["query"]["pages"][0]["imageinfo"][0];
        file["width"] = json!(800);
        file["thumburl"] = json!(ORIGINAL);
        table[0].1 = serde_json::to_vec(&info).unwrap();
        table.truncate(4);
        table.push((ORIGINAL.into(), tiny_jpeg(800, 571)));
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"commons","object":"File:Rembrandt - The laughing man.jpg","max_requests":8,"files":true});
        let result = observe_with(&request, table).unwrap();
        let image = &result["observation"]["images"][0];
        assert_eq!(image["file_url"], ORIGINAL);
        assert_eq!(image["preview_url"], ORIGINAL);
        assert_eq!(image["preview_fingerprint"], image["file_fingerprint"]);
        assert_eq!(image["file_fingerprint"]["width_pixels"], 800);
        assert_eq!(
            roles(&result).last().map(String::as_str),
            Some("image-file")
        );
        assert!(
            result["observation"]["warnings"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_commons_file_must_name_the_artwork_it_represents() {
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"commons","object":"File:Rembrandt - The laughing man.jpg","max_requests":8});
        let error = observe_with(&request, commons_table("Q19652", None)).unwrap_err();
        assert!(error.contains("P6243"), "{error}");
        let bad = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"commons","object":"File:a|b.jpg","max_requests":8});
        assert!(
            observe_with(&bad, Vec::new())
                .unwrap_err()
                .contains("named by its title")
        );
    }

    #[test]
    fn slam_page_states_fields_rights_and_original() {
        const PAGE: &str = "https://www.slam.org/collection/objects/36470/";
        const ORIGINAL: &str =
            "https://d23gjylftzdlen.cloudfront.net/db_photo/d3363/u16333630/781945dig_1_o2.jpg";
        const COPY: &str =
            "https://d23gjylftzdlen.cloudfront.net/db_photo/d3363/u16333630/781945dig_1_o3.jpg";
        let html = format!(
            r#"<html><body><h1 class="title">Athlete Wrestling with a Python</h1>
            <dl><dt>Artist</dt><dd>Frederic Leighton, English, 1830&#8211;1896</dd>
            <dt>Date</dt><dd>1877</dd><dt>Medium</dt><dd>bronze</dd><dt>Classification</dt><dd>Sculpture</dd>
            <dt>Dimensions</dt><dd>height including base: 20 5/16 in. (51.6 cm)</dd>
            <dt>Credit Line</dt><dd>Museum Purchase</dd><dt>Object Number</dt><dd>78:1945</dd>
            <dt>Rights</dt><dd>Public Domain</dd></dl>
            <script>var VIEWER_MAIN_IMG = {{ url: '{ORIGINAL}' }};</script></body></html>"#
        );
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"slam","object":"36470","max_requests":4,"files":true});
        let result = observe_with(
            &request,
            vec![
                (PAGE.into(), html.into_bytes()),
                (ORIGINAL.into(), tiny_jpeg(4500, 6000)),
                (COPY.into(), tiny_jpeg(1200, 1600)),
            ],
        )
        .unwrap();
        let observation = &result["observation"];
        assert_eq!(observation["object"]["identifiers"][0]["value"], "78:1945");
        assert_eq!(
            observation["object"]["titles"][0]["text"],
            "Athlete Wrestling with a Python"
        );
        assert_eq!(
            observation["object"]["production_statements"][0]["text"],
            "Frederic Leighton, English, 1830–1896"
        );
        assert_eq!(
            observation["object"]["dating"],
            json!({"statement":"1877","begin_year":1877,"end_year":1877})
        );
        assert_eq!(
            observation["rights"],
            json!([{"scope":"image","provider_field":"Rights","value":"Public Domain","statement_uri":null,"label":"Public Domain","stated_in":PAGE}])
        );
        assert_eq!(observation["images"][0]["file_url"], ORIGINAL);
        assert_eq!(observation["images"][0]["preview_url"], COPY);
        assert_eq!(
            roles(&result),
            ["object-record", "image-preview", "image-file"]
        );
    }

    fn smb_page(original: &str) -> String {
        format!(
            r#"<html><head><style>.gallery-image-frame-single{{display:flex}}</style></head><body><main>
            <h1 class="text-3xl">Bildnis Friedrich Nietzsche</h1><p>Ident. Nr.: A II 898</p>
            <div><p class="font-bold">Einrichtung:</p><p class="font-normal">Alte Nationalgalerie</p></div>
            <dl><dt class="x">Beteiligte</dt><dd class="y"><span>Curt Stoeving (1863 - 1939), Maler</span></dd>
            <dt>Datierung</dt><dd><span>1894</span></dd><dt>Systematik/Art</dt><dd>Gemälde</dd>
            <dt>Material / Technik</dt><dd>Öl auf Leinwand</dd><dt>Abmessungen</dt><dd>Höhe x Breite: 180 x 242 cm</dd></dl>
            <div id="gallery-photoswipe"><div class="embla__slide"><div class="gallery-image-frame-single">
            <a href="{original}" data-pswp-width="1200"><img src="/media/images/thumbs/extra_large/157/005/5157106.jpg" class="gallery-image-single"></a></div>
            <div class="text-center text-xs">Fotonachweis: Staatliche Museen zu Berlin, Alte Nationalgalerie / Klaus Göken Public Domain Mark 1.0</div></div></div>
            </main></body></html>"#
        )
    }

    #[test]
    fn smb_page_states_fields_photo_credit_and_original() {
        const PAGE: &str = "https://search.smb.museum/object/obj-965972";
        const ORIGINAL: &str =
            "https://search.smb.museum/media/images/originals/157/005/5157106.AII898_002mR.jpg";
        const COPY: &str =
            "https://search.smb.museum/media/images/thumbs/extra_large/157/005/5157106.jpg";
        let request = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"smb","object":"obj-965972","max_requests":4,"files":true});
        let page = smb_page("/media/images/originals/157/005/5157106.AII898_002mR.jpg");
        let result = observe_with(
            &request,
            vec![
                (PAGE.into(), page.clone().into_bytes()),
                (ORIGINAL.into(), tiny_jpeg(3000, 2230)),
                (COPY.into(), tiny_jpeg(1200, 892)),
            ],
        )
        .unwrap();
        let observation = &result["observation"];
        let object = &observation["object"];
        assert_eq!(
            object["identifiers"],
            json!([{"scheme":"SMB ObjID","value":"965972"},{"scheme":"SMB inventory number","value":"A II 898"}])
        );
        assert_eq!(
            object["repository"],
            "Staatliche Museen zu Berlin, Alte Nationalgalerie"
        );
        assert_eq!(
            object["production_statements"][0]["text"],
            "Curt Stoeving (1863 - 1939), Maler"
        );
        assert_eq!(
            object["dimensions"][1],
            json!({"kind":"height","value":"180","unit":"cm","unit_uri":null,"statement":null})
        );
        assert_eq!(object["dimensions"][2]["value"], "242");
        assert_eq!(
            observation["rights"][0]["statement_uri"],
            "https://creativecommons.org/publicdomain/mark/1.0/"
        );
        assert_eq!(observation["rights"][0]["stated_in"], PAGE);
        assert_eq!(observation["images"][0]["file_url"], ORIGINAL);
        assert_eq!(observation["images"][0]["preview_url"], COPY);
        assert_eq!(
            roles(&result),
            ["object-record", "image-preview", "image-file"]
        );

        // An original larger than one response may hold stays at the museum; its 1200-pixel copy is the file.
        let mut huge = tiny_jpeg(9000, 7000);
        huge.resize(record_limit().unwrap() + 1, 0);
        let result = observe_with(
            &request,
            vec![
                (PAGE.into(), page.into_bytes()),
                (ORIGINAL.into(), huge),
                (COPY.into(), tiny_jpeg(1200, 892)),
            ],
        )
        .unwrap();
        let image = &result["observation"]["images"][0];
        assert_eq!(image["file_url"], COPY);
        assert_eq!(image["file_fingerprint"]["width_pixels"], 1200);
        assert!(
            result["observation"]["warnings"][0]
                .as_str()
                .unwrap()
                .contains("exceeds the record limit")
        );

        // A credit without the mark states no open statement.
        let closed = smb_page("/media/images/originals/157/005/5157106.AII898_002mR.jpg").replace(
            " Public Domain Mark 1.0",
            " / Foto: Alle Rechte vorbehalten",
        );
        let light = json!({"operation":"media.observe","repo_root":repo_root(),"provider":"smb","object":"965972","max_requests":4});
        let result = observe_with(&light, vec![(PAGE.into(), closed.into_bytes())]).unwrap();
        assert_eq!(
            result["observation"]["rights"][0]["statement_uri"],
            Value::Null
        );
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
                "media-provider-observation.v1.nga.json",
                "media-provider-observation",
            ),
            (
                "media-provider-observation.v1.commons.json",
                "media-provider-observation",
            ),
            (
                "media-provider-observation.v1.slam.json",
                "media-provider-observation",
            ),
            (
                "media-provider-observation.v1.smb.json",
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
