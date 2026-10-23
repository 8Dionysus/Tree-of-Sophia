//! Synthetic profile and custody checks. No private caller or corpus is used.

use super::tests::{djvu_page, stored_zip_archive};
use super::*;
use flate2::{Compression, write::GzEncoder};
use std::io::Write;
use std::sync::atomic::Ordering;

fn config(raw: &[u8], media: &str, basename: &str) -> Value {
    let digest = Digest256::of_bytes(raw).to_hex();
    json!({"file_id":format!("tos.file.sha256.{digest}"),"sha256":digest,
        "byte_size":raw.len(),"media_type":media,"payload_basename":basename,
        "original_basename":basename,"input_path":"/ungranted/no-source-here"})
}

fn held(raw: &[u8]) -> File {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(raw).unwrap();
    file
}

fn end() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn observe(raw: &[u8], media: &str, basename: &str) -> SourceCommandResult<Value> {
    super::super::observe(
        &mut held(raw),
        &config(raw, media, basename),
        end(),
        &AtomicBool::new(false),
        &mut || Ok(()),
    )
}

#[test]
fn held_dispatch_reuses_every_native_profile_after_named_path_replacement() {
    let tei = br#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body><div><head>Private phrase</head><p>Private phrase</p></div></body></text></TEI>"#;
    let osis = br#"<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace"><osisText><div><chapter osisID="Gen.1"><verse osisID="Gen.1.1"><w>Private phrase</w></verse></chapter></div></osisText></osis>"#;
    let djvu_xml = br#"<DjVuXML><BODY><OBJECT width="1200" height="1800"><PARAM name="DPI" value="300"/><PARAGRAPH><LINE><WORD>Private phrase</WORD></LINE></PARAGRAPH></OBJECT></BODY></DjVuXML>"#;
    let scandata = br#"<book><bookData><leafCount>1</leafCount><dpi>300</dpi></bookData><pageData><page leafNum="0"><origWidth>100</origWidth><origHeight>200</origHeight></page></pageData></book>"#;
    let abbyy = br#"<document xmlns="http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml"><page width="1200" height="1800" resolution="300"><par><line><charParams wordStart="true">Private phrase</charParams></line></par></page></document>"#;
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(abbyy).unwrap();
    let container = br#"<container><rootfiles><rootfile full-path="EPUB/package.opf"/></rootfiles></container>"#;
    let package = br#"<package><manifest><item id="p1" href="p.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="p1"/></spine></package>"#;
    let fixtures = vec![
        (
            "application/json",
            "file.json",
            br#"{"label":"Private phrase","count":3}"#.to_vec(),
            "json_members_v1",
        ),
        (
            "application/tei+xml",
            "file.xml",
            tei.to_vec(),
            "tei_structure_v1",
        ),
        (
            "application/osis+xml",
            "file.xml",
            osis.to_vec(),
            "osis_structure_v1",
        ),
        (
            "application/xml",
            "file.xml",
            tei.to_vec(),
            "tei_structure_v1",
        ),
        ("text/xml", "file.xml", osis.to_vec(), "osis_structure_v1"),
        (
            "application/vnd.djvu+xml",
            "file.djvu.xml",
            djvu_xml.to_vec(),
            "djvu_xml_pages_v1",
        ),
        (
            "application/xml",
            "file_scandata.xml",
            scandata.to_vec(),
            "scandata_pages_v1",
        ),
        (
            "image/vnd.djvu",
            "file.djvu",
            djvu_page(100, 200, 300),
            "djvu_pages_v1",
        ),
        (
            "application/gzip",
            "file.abbyy.xml.gz",
            gzip.finish().unwrap(),
            "abbyy_xml_pages_v1",
        ),
        (
            "application/zip",
            "file_jp2.zip",
            stored_zip_archive(&[("file/page_0000.jp2", b"Private phrase")]),
            "jp2_zip_pages_v1",
        ),
        (
            "application/epub+zip",
            "file.epub",
            stored_zip_archive(&[
                ("mimetype", b"application/epub+zip"),
                ("META-INF/container.xml", container),
                ("EPUB/package.opf", package),
                ("EPUB/p.xhtml", b"<p>Private phrase</p>"),
            ]),
            "epub_resources_v1",
        ),
        (
            "text/plain",
            "file.txt",
            b"Private phrase\r\n".to_vec(),
            "plain_utf8_file_v1",
        ),
        (
            "text/markdown",
            "file.md",
            b"# Private phrase\n".to_vec(),
            "plain_utf8_file_v1",
        ),
    ];
    for (media, name, raw, profile) in fixtures {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join(name);
        fs::write(&path, &raw).unwrap();
        let mut file = File::open(&path).unwrap();
        let mut configuration = config(&raw, media, name);
        configuration["input_path"] = json!(path);
        let entry = payload_entry(&configuration).unwrap();
        let expected = legacy_file_inventory(
            &path,
            &raw,
            &entry,
            "plain_utf8_file_v1",
            end(),
            &AtomicBool::new(false),
        )
        .unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"different ungranted bytes").unwrap();
        let actual = super::super::observe(
            &mut file,
            &configuration,
            end(),
            &AtomicBool::new(false),
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            actual, expected,
            "existing profile contract: {media}/{name}"
        );
        assert_eq!(actual["profile"], profile);
        let rendered = serde_json::to_string(&actual).unwrap();
        assert!(!rendered.contains("Private phrase"));
        assert!(!rendered.contains("different ungranted bytes"));
        assert!(!rendered.contains(temporary.path().to_str().unwrap()));
    }
}

#[test]
fn held_profiles_refuse_changed_size_hash_suffix_and_existing_byte_limits() {
    let raw = br#"{"value":"Private phrase"}"#;
    for (field, value) in [
        ("sha256", json!("0".repeat(64))),
        ("byte_size", json!(raw.len() + 1)),
    ] {
        let mut configuration = config(raw, "application/json", "file.json");
        configuration[field] = value;
        assert!(matches!(
            super::super::observe(
                &mut held(raw),
                &configuration,
                end(),
                &AtomicBool::new(false),
                &mut || Ok(()),
            ),
            Err(SourceCommandError::Conflict(_))
        ));
    }
    for (media, basename) in [
        ("application/zip", "generic.zip"),
        ("application/gzip", "generic.gz"),
    ] {
        assert!(matches!(
            observe(b"not interpreted by extension", media, basename),
            Err(SourceCommandError::Unsupported(_))
        ));
    }
    for (media, maximum) in [
        ("text/plain", MAX_PLAIN_UTF8_BYTES as u64),
        (
            "application/json",
            super::super::MAX_INVENTORY_JSON_BYTES as u64,
        ),
        ("image/vnd.djvu", MAX_PROFILE_BYTES),
    ] {
        let mut configuration = config(raw, media, "file.bin");
        configuration["byte_size"] = json!(maximum + 1);
        assert!(matches!(
            super::super::observe(
                &mut held(raw),
                &configuration,
                end(),
                &AtomicBool::new(false),
                &mut || Ok(()),
            ),
            Err(SourceCommandError::Unsupported(_))
        ));
    }
}

#[test]
fn authorization_deadline_and_cancellation_survive_parser_refusal() {
    let raw = b"<TEI xmlns='http://www.tei-c.org/ns/1.0'><text><body><div>Private phrase</div></body></text></TEI>";
    let configuration = config(raw, "application/tei+xml", "file.xml");
    let mut checks = 0;
    let error = super::super::observe(
        &mut held(raw),
        &configuration,
        end(),
        &AtomicBool::new(false),
        &mut || {
            checks += 1;
            if checks >= 12 {
                Err(SourceCommandError::Denied("revoked profile input"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SourceCommandError::Denied("revoked profile input")
    ));
    assert_eq!(checks, 12);
    for (deadline, cancelled) in [
        (Instant::now(), AtomicBool::new(false)),
        (end(), AtomicBool::new(true)),
    ] {
        let mut calls = 0;
        assert!(
            super::super::observe_with_pdf(
                &mut held(b"%PDF-fixture"),
                &config(b"%PDF-fixture", "application/pdf", "file.pdf"),
                deadline,
                &cancelled,
                &mut || Ok(()),
                &mut |_, _, _, _| {
                    calls += 1;
                    Ok(vec![])
                },
            )
            .is_err()
        );
        assert_eq!(calls, 0);
    }
    let cancelled = AtomicBool::new(false);
    let mut checks = 0;
    assert!(
        super::super::observe(
            &mut held(raw),
            &configuration,
            end(),
            &cancelled,
            &mut || {
                checks += 1;
                if checks == 12 {
                    cancelled.store(true, Ordering::Relaxed);
                }
                Ok(())
            },
        )
        .is_err()
    );
    assert_eq!(checks, 12);
}

fn pdf_output(probe: super::super::PdfInventoryProbe) -> Vec<u8> {
    use super::super::PdfInventoryProbe::*;
    match probe {
        PageCount => b"Pages: 2\n".to_vec(),
        PageBoxes { page_count: 2 } => b"Page 1 size: 100 x 200 pts\nPage 1 rot: 0\nPage 2 size: 100 x 200 pts\nPage 2 rot: 90\n".to_vec(),
        PageBoxes { .. } => panic!("unexpected page count"),
        Images => b"1 0 image\n2 1 smask\n".to_vec(),
    }
}

#[test]
fn pdf_dispatch_uses_fixed_probes_exact_bytes_and_bounded_navigation_only() {
    use super::super::PdfInventoryProbe;
    let raw = b"%PDF-fixture Private phrase";
    let mut probes = Vec::new();
    let inventory = super::super::observe_with_pdf(
        &mut held(raw),
        &config(raw, "application/pdf", "file.pdf"),
        end(),
        &AtomicBool::new(false),
        &mut || Ok(()),
        &mut |probe, input, cap, check| {
            check()?;
            assert_eq!(input, raw);
            assert_eq!(cap, MAX_EXTERNAL_STDOUT);
            probes.push(probe);
            Ok(pdf_output(probe))
        },
    )
    .unwrap();
    assert_eq!(
        probes,
        vec![
            PdfInventoryProbe::PageCount,
            PdfInventoryProbe::PageBoxes { page_count: 2 },
            PdfInventoryProbe::Images
        ]
    );
    assert_eq!(inventory["profile"], "pdf_pages_v1");
    assert_eq!(inventory["summary"]["page_count"], 2);
    assert_eq!(inventory["summary"]["image_resource_count"], 2);
    assert_eq!(inventory["resources"][1]["locator"]["rotation_degrees"], 90);
    assert!(
        !serde_json::to_string(&inventory)
            .unwrap()
            .contains("Private phrase")
    );
}

#[test]
fn pdf_provider_cannot_hide_revocation_mutation_or_unbounded_output() {
    let raw = b"%PDF-fixture Private phrase";
    let configuration = config(raw, "application/pdf", "file.pdf");
    let revoked = AtomicBool::new(false);
    let result = super::super::observe_with_pdf(
        &mut held(raw),
        &configuration,
        end(),
        &AtomicBool::new(false),
        &mut || {
            if revoked.load(Ordering::Relaxed) {
                Err(SourceCommandError::Denied("revoked PDF input"))
            } else {
                Ok(())
            }
        },
        &mut |_, _, _, _| {
            revoked.store(true, Ordering::Relaxed);
            Err(profile_error("PDF parser refused"))
        },
    );
    assert!(matches!(
        result,
        Err(SourceCommandError::Denied("revoked PDF input"))
    ));
    let mut file = held(raw);
    let mut writer = file.try_clone().unwrap();
    let result = super::super::observe_with_pdf(
        &mut file,
        &configuration,
        end(),
        &AtomicBool::new(false),
        &mut || Ok(()),
        &mut |probe, _, _, _| {
            writer.seek(SeekFrom::Start(0)).unwrap();
            writer.write_all(&vec![b'x'; raw.len()]).unwrap();
            writer.flush().unwrap();
            Ok(pdf_output(probe))
        },
    );
    assert!(matches!(result, Err(SourceCommandError::Conflict(_))));
    let result = super::super::observe_with_pdf(
        &mut held(raw),
        &configuration,
        end(),
        &AtomicBool::new(false),
        &mut || Ok(()),
        &mut |_, _, cap, _| Ok(vec![b'x'; cap + 1]),
    );
    assert!(matches!(
        result,
        Err(SourceCommandError::Unsupported(
            "bounded PDF inventory provider output"
        ))
    ));
    let mut probes = 0;
    let result = super::super::observe_with_pdf(
        &mut held(raw),
        &configuration,
        end(),
        &AtomicBool::new(false),
        &mut || Ok(()),
        &mut |_, _, _, _| {
            probes += 1;
            Ok(format!("Pages: {}\n", usize::MAX).into_bytes())
        },
    );
    assert!(matches!(
        result,
        Err(SourceCommandError::Unsupported("bounded PDF page count"))
    ));
    assert_eq!(probes, 1);
}

#[test]
fn synthetic_pdf_executes_the_existing_native_parsers_on_exact_stdin() {
    use super::super::PdfInventoryProbe;
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut raw = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(raw.len());
        write!(raw, "{} 0 obj\n{object}\nendobj\n", index + 1).unwrap();
    }
    let xref = raw.len();
    write!(raw, "xref\n0 5\n0000000000 65535 f \n").unwrap();
    for offset in offsets {
        writeln!(raw, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        raw,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    let deadline = end();
    let cancelled = AtomicBool::new(false);
    let configuration = config(&raw, "application/pdf", "synthetic.pdf");
    let inventory = super::super::observe_with_pdf(
        &mut held(&raw),
        &configuration,
        deadline,
        &cancelled,
        &mut || Ok(()),
        &mut |probe, input, cap, check| {
            check()?;
            assert_eq!(input, raw);
            let (program, args) = match probe {
                PdfInventoryProbe::PageCount => ("/usr/bin/pdfinfo", vec!["-".to_owned()]),
                PdfInventoryProbe::PageBoxes { page_count } => (
                    "/usr/bin/pdfinfo",
                    vec![
                        "-f".into(),
                        "1".into(),
                        "-l".into(),
                        page_count.to_string(),
                        "-box".into(),
                        "-".into(),
                    ],
                ),
                PdfInventoryProbe::Images => {
                    ("/usr/bin/pdfimages", vec!["-list".into(), "-".into()])
                }
            };
            let output = crate::source_text_owner_ocr::bounded_process_with_stdin(
                program,
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
                None,
                Some(input),
                cap,
                deadline,
                &cancelled,
            )?;
            check()?;
            Ok(output)
        },
    )
    .unwrap();
    assert_eq!(inventory["profile"], "pdf_pages_v1");
    assert_eq!(inventory["summary"]["page_count"], 1);
    assert_eq!(inventory["resources"][0]["locator"]["width_points"], 200.0);
    assert_eq!(inventory["resources"][0]["locator"]["height_points"], 300.0);
}
