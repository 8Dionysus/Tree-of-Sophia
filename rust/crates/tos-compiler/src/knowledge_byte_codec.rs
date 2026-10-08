//! Bounded byte compression for an explicitly selected physical model format.
//! This module supplies no model or source authority. Callers authenticate the
//! format and verify logical length/digest before delivering decoded contents.
use crate::{Error, Result, d1_public_capture::CreationState};
use flate2::{Compress, Compression, FlushCompress, Status};
use miniz_oxide::inflate::{
    TINFLStatus,
    core::{DecompressorOxide, decompress, inflate_flags},
};

const MAGIC: &[u8; 8] = b"TOSBYT2\0";
pub(crate) const HEADER: usize = 17;
const CHUNK: usize = 4096;

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or(Error::Budget("byte codec size overflow"))
}

fn encoder_workspace_upper() -> Result<usize> {
    // Pinned flate2 1.1.10/miniz_oxide 0.9.1 rust_backend. CompressorOxide
    // contains pointers to HuffmanOxide, LocalBuf and three HashBuffers;
    // sizeof(CompressorOxide) alone omits those allocations. Include both
    // retained storage and construction frames. WASM additionally boxes codes.
    let nested = 3 * 288 * (2 + 2 + 1)
        + (65536 * 13) / 10
        + (32768 + 258)
        + 2 * 32768 * 2
        + if cfg!(target_arch = "wasm32") {
            65536
        } else {
            0
        };
    add(
        std::mem::size_of::<miniz_oxide::deflate::core::CompressorOxide>(),
        nested,
    )?
    .checked_mul(2)
    .and_then(|n| n.checked_add(16 * 1024 + std::mem::size_of::<Compress>()))
    .ok_or(Error::Budget("byte encoder workspace"))
}

/// The selected V2 ABI frames every blob, including incompressible bytes.
/// The codec selector is authenticated format data, never guessed from source.
pub(crate) fn stored_bound(max_logical_bytes: usize) -> Result<usize> {
    add(max_logical_bytes, HEADER)
}

pub(crate) fn with_encoded<T>(
    state: &CreationState<'_>,
    raw: &[u8],
    max_bytes: usize,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    state.active()?;
    if raw.is_empty() || raw.len() > max_bytes {
        return Err(Error::Budget("byte encoder input"));
    }
    let output_cap = stored_bound(raw.len())?;
    let frame = add(
        std::mem::size_of_val(&consume),
        std::mem::size_of::<(Vec<u8>, usize, usize, usize, usize, Status, Result<T>)>(),
    )?;
    let _hold = state.hold(add(add(encoder_workspace_upper()?, output_cap)?, frame)?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_cap)
        .map_err(|_| Error::Budget("byte encoder allocation"))?;
    if output.capacity() != output_cap {
        return Err(Error::Budget("byte encoder allocation capacity"));
    }
    state.charge_work(output_cap)?;
    output.resize(output_cap, 0);
    let used = encode_into(raw, &mut output, |bytes| {
        state.active()?;
        state.charge_work(bytes)
    })?;
    state.active()?;
    consume(&output[..used])
}

/// The caller admits the complete output and backend workspace first.
fn encode_into(
    raw: &[u8],
    output: &mut [u8],
    mut checkpoint: impl FnMut(usize) -> Result<()>,
) -> Result<usize> {
    checkpoint(0)?;
    if raw.is_empty() || output.len() != stored_bound(raw.len())? {
        return Err(Error::Invalid("byte encoder output admission"));
    }
    output[..8].copy_from_slice(MAGIC);
    output[8] = 1;
    output[9..HEADER].copy_from_slice(&(raw.len() as u64).to_le_bytes());
    let mut encoder = Compress::new(Compression::default(), true);
    let mut input_at = 0usize;
    let mut output_at = HEADER;
    loop {
        checkpoint(0)?;
        if output_at == output.len() {
            break;
        }
        let input_end = raw.len().min(add(input_at, CHUNK)?);
        let output_end = output.len().min(add(output_at, CHUNK)?);
        checkpoint(add(input_end - input_at, output_end - output_at)?)?;
        let before_in = encoder.total_in();
        let before_out = encoder.total_out();
        let status = encoder
            .compress(
                &raw[input_at..input_end],
                &mut output[output_at..output_end],
                if input_end == raw.len() {
                    FlushCompress::Finish
                } else {
                    FlushCompress::None
                },
            )
            .map_err(|_| Error::Invalid("byte encoder stream"))?;
        let used = usize::try_from(encoder.total_in() - before_in)
            .map_err(|_| Error::Invalid("byte encoder input count"))?;
        let made = usize::try_from(encoder.total_out() - before_out)
            .map_err(|_| Error::Invalid("byte encoder output count"))?;
        if used > input_end - input_at || made > output_end - output_at {
            return Err(Error::Invalid("byte encoder counts"));
        }
        input_at += used;
        output_at += made;
        checkpoint(0)?;
        if status == Status::StreamEnd {
            if input_at != raw.len() {
                return Err(Error::Invalid("byte encoder incomplete input"));
            }
            if output_at < output.len() {
                return Ok(output_at);
            }
            break;
        }
        if used == 0 && made == 0 {
            return Err(Error::Invalid("byte encoder stalled"));
        }
    }
    // Same framed format when compression does not save space.
    output[8] = 0;
    checkpoint(raw.len())?;
    output[HEADER..].copy_from_slice(raw);
    checkpoint(0)?;
    Ok(output.len())
}

/// Validate the complete physical envelope before any decoded allocation.
/// Return (logical byte length, compressed). The caller verifies logical hash.
pub(crate) fn frame_metadata(
    stored: &[u8],
    expected_bytes: Option<usize>,
    max_bytes: usize,
) -> Result<(usize, bool)> {
    if stored.len() <= HEADER || stored.len() > stored_bound(max_bytes)? || &stored[..8] != MAGIC {
        return Err(Error::Invalid("byte decoder envelope"));
    }
    let decoded_len = usize::try_from(u64::from_le_bytes(
        stored[9..HEADER]
            .try_into()
            .map_err(|_| Error::Invalid("byte decoder length"))?,
    ))
    .map_err(|_| Error::Budget("byte decoder length"))?;
    if decoded_len == 0
        || decoded_len > max_bytes
        || expected_bytes.is_some_and(|expected| expected != decoded_len)
    {
        return Err(Error::Budget("byte decoder declared length"));
    }
    let compressed = match stored[8] {
        0 if stored.len() == stored_bound(decoded_len)? => false,
        1 if stored.len() < stored_bound(decoded_len)? => true,
        _ => return Err(Error::Invalid("byte decoder codec or stored length")),
    };
    Ok((decoded_len, compressed))
}

pub(crate) fn decoder_workspace_upper() -> Result<usize> {
    // Core inflate retains its dictionary in the caller's result buffer.
    std::mem::size_of::<DecompressorOxide>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(16 * 1024))
        .ok_or(Error::Budget("byte decoder workspace"))
}

/// Allocation-free stream core for the existing owned and page-bounded readers.
/// The caller admits output plus decoder_workspace_upper before entering and
/// owns the work/deadline callback. This does not create an operation budget.
/// Output includes one admitted spare byte to detect understated raw length.
pub(crate) fn decompress_into(
    stored: &[u8],
    decoded_len: usize,
    output: &mut [u8],
    mut checkpoint: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    checkpoint(0)?;
    if frame_metadata(stored, Some(decoded_len), decoded_len)? != (decoded_len, true)
        || output.len() != add(decoded_len, 1)?
    {
        return Err(Error::Invalid("byte decoder output admission"));
    }
    let output_cap = output.len();
    let mut decoder = DecompressorOxide::new();
    let input = &stored[HEADER..];
    let mut input_at = 0usize;
    let mut output_at = 0usize;
    loop {
        checkpoint(0)?;
        let input_end = input.len().min(add(input_at, CHUNK)?);
        let output_end = output_cap.min(add(output_at, CHUNK)?);
        if output_at == output_cap {
            return Err(Error::Invalid("byte decoder excess output"));
        }
        checkpoint(add(input_end - input_at, output_end - output_at)?)?;
        let flags = inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | inflate_flags::TINFL_FLAG_COMPUTE_ADLER32
            | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | if input_end < input.len() {
                inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
            } else {
                0
            };
        let (status, used, made) = decompress(
            &mut decoder,
            &input[input_at..input_end],
            &mut output[..output_end],
            output_at,
            flags,
        );
        if used > input_end - input_at || made > output_end - output_at {
            return Err(Error::Invalid("byte decoder counts"));
        }
        input_at += used;
        output_at += made;
        checkpoint(0)?;
        match status {
            TINFLStatus::Done => {
                if input_at != input.len() || output_at != decoded_len {
                    return Err(Error::Invalid("byte decoder exact EOF"));
                }
                return Ok(());
            }
            TINFLStatus::NeedsMoreInput | TINFLStatus::HasMoreOutput => {}
            _ => return Err(Error::Invalid("byte decoder stream")),
        }
        if used == 0 && made == 0 {
            return Err(Error::Invalid("byte decoder stalled"));
        }
    }
}

/// Decode under the original CreationState and retain output through consume.
pub(crate) fn with_decoded<T>(
    state: &CreationState<'_>,
    stored: &[u8],
    expected_bytes: Option<usize>,
    max_bytes: usize,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    state.active()?;
    let (decoded_len, compressed) = frame_metadata(stored, expected_bytes, max_bytes)?;
    if !compressed {
        return consume(&stored[HEADER..]);
    }
    let output_cap = add(decoded_len, 1)?;
    let frame = add(
        std::mem::size_of_val(&consume),
        std::mem::size_of::<(Vec<u8>, usize, usize, Result<T>)>(),
    )?;
    let _hold = state.hold(add(add(output_cap, decoder_workspace_upper()?)?, frame)?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_cap)
        .map_err(|_| Error::Budget("byte decoder allocation"))?;
    if output.capacity() != output_cap {
        return Err(Error::Budget("byte decoder allocation capacity"));
    }
    state.charge_work(output_cap)?;
    output.resize(output_cap, 0);
    decompress_into(stored, decoded_len, &mut output, |work| {
        state.active()?;
        state.charge_work(work)
    })?;
    state.active()?;
    consume(&output[..decoded_len])
}

// V3 adds an explicitly selected, content-addressed preset dictionary. V2
// frames remain valid inside V3; a V2 consumer still rejects this new magic.
pub(crate) const DICTIONARY_BYTES: usize = 4096;
const DICTIONARY_MAGIC: &[u8; 8] = b"TOSBYT3\0";
const DICTIONARY_HEADER: usize = HEADER + 32;

pub(crate) fn is_dictionary_frame(stored: &[u8]) -> bool {
    stored.starts_with(DICTIONARY_MAGIC)
}

pub(crate) fn dictionary_frame_metadata(
    stored: &[u8],
    expected: Option<usize>,
    max_bytes: usize,
) -> Result<(usize, tos_foundation::Digest256)> {
    if stored.len() <= DICTIONARY_HEADER
        || stored.len() > stored_bound(max_bytes)?
        || !is_dictionary_frame(stored)
        || stored[8] != 2
    {
        return Err(Error::Invalid("dictionary byte envelope"));
    }
    let length = usize::try_from(u64::from_le_bytes(
        stored[9..HEADER]
            .try_into()
            .map_err(|_| Error::Invalid("dictionary byte length"))?,
    ))
    .map_err(|_| Error::Budget("dictionary byte length"))?;
    if length == 0
        || length > max_bytes
        || expected.is_some_and(|n| n != length)
        || stored.len() > stored_bound(length)?
    {
        return Err(Error::Invalid("dictionary byte declared length"));
    }
    let digest = tos_foundation::Digest256::from_bytes(
        stored[HEADER..DICTIONARY_HEADER]
            .try_into()
            .map_err(|_| Error::Invalid("dictionary digest length"))?,
    );
    Ok((length, digest))
}

pub(crate) fn with_dictionary_encoded<T>(
    state: &CreationState<'_>,
    raw: &[u8],
    dictionary: &[u8],
    max_bytes: usize,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    state.active()?;
    if raw.is_empty()
        || raw.len() > max_bytes
        || dictionary.is_empty()
        || dictionary.len() > DICTIONARY_BYTES
    {
        return Err(Error::Budget("dictionary encoder input"));
    }
    let cap = stored_bound(raw.len())?;
    let workspace = add(encoder_workspace_upper()?, CHUNK + 2 * DICTIONARY_BYTES)?;
    let _hold = state.hold(add(add(workspace, cap)?, std::mem::size_of_val(&consume))?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(cap)
        .map_err(|_| Error::Budget("dictionary encoder allocation"))?;
    if output.capacity() != cap {
        return Err(Error::Budget("dictionary encoder capacity"));
    }
    state.charge_work(cap)?;
    output.resize(cap, 0);
    let used = encode_dictionary_into(raw, dictionary, &mut output, |bytes| {
        state.active()?;
        state.charge_work(bytes)
    })?;
    state.active()?;
    consume(&output[..used])
}

fn encode_dictionary_into(
    raw: &[u8],
    dictionary: &[u8],
    output: &mut [u8],
    mut checkpoint: impl FnMut(usize) -> Result<()>,
) -> Result<usize> {
    checkpoint(0)?;
    if raw.is_empty()
        || dictionary.is_empty()
        || dictionary.len() > DICTIONARY_BYTES
        || output.len() != stored_bound(raw.len())?
    {
        return Err(Error::Invalid("dictionary encoder admission"));
    }
    if output.len() > DICTIONARY_HEADER {
        output[..8].copy_from_slice(DICTIONARY_MAGIC);
        output[8] = 2;
        output[9..HEADER].copy_from_slice(&(raw.len() as u64).to_le_bytes());
        checkpoint(dictionary.len())?;
        output[HEADER..DICTIONARY_HEADER]
            .copy_from_slice(tos_foundation::Digest256::of_bytes(dictionary).as_bytes());
        let mut encoder = Compress::new(Compression::default(), false);
        // Sync ends on a byte boundary without clearing the existing miniz
        // history. The discarded prefix initializes exactly the dictionary
        // supplied to the bounded raw-deflate decoder below. No backend switch.
        let mut discard = [0u8; CHUNK];
        let mut at = 0;
        loop {
            checkpoint(dictionary.len() - at + discard.len())?;
            let before_in = encoder.total_in();
            let before_out = encoder.total_out();
            let status = encoder
                .compress(&dictionary[at..], &mut discard, FlushCompress::Sync)
                .map_err(|_| Error::Invalid("dictionary encoder prefix"))?;
            let used = (encoder.total_in() - before_in) as usize;
            let made = (encoder.total_out() - before_out) as usize;
            if used > dictionary.len() - at || made > discard.len() || status == Status::StreamEnd {
                return Err(Error::Invalid("dictionary encoder prefix counts"));
            }
            at += used;
            checkpoint(0)?;
            if at == dictionary.len() && made < discard.len() {
                break;
            }
            if used == 0 && made == 0 {
                return Err(Error::Invalid("dictionary encoder prefix stalled"));
            }
        }
        let mut input_at = 0;
        let mut output_at = DICTIONARY_HEADER;
        loop {
            checkpoint(0)?;
            if output_at == output.len() {
                break;
            }
            let input_end = raw.len().min(add(input_at, CHUNK)?);
            let output_end = output.len().min(add(output_at, CHUNK)?);
            checkpoint(input_end - input_at + output_end - output_at)?;
            let before_in = encoder.total_in();
            let before_out = encoder.total_out();
            let status = encoder
                .compress(
                    &raw[input_at..input_end],
                    &mut output[output_at..output_end],
                    if input_end == raw.len() {
                        FlushCompress::Finish
                    } else {
                        FlushCompress::None
                    },
                )
                .map_err(|_| Error::Invalid("dictionary encoder stream"))?;
            let used = (encoder.total_in() - before_in) as usize;
            let made = (encoder.total_out() - before_out) as usize;
            if used > input_end - input_at || made > output_end - output_at {
                return Err(Error::Invalid("dictionary encoder stream counts"));
            }
            input_at += used;
            output_at += made;
            checkpoint(0)?;
            if status == Status::StreamEnd {
                if input_at != raw.len() {
                    return Err(Error::Invalid("dictionary encoder incomplete"));
                }
                if output_at < output.len() {
                    return Ok(output_at);
                }
                break;
            }
            if used == 0 && made == 0 {
                return Err(Error::Invalid("dictionary encoder stalled"));
            }
        }
    }
    // Short or incompressible rows retain the unchanged bounded V2 raw frame.
    output[..8].copy_from_slice(MAGIC);
    output[8] = 0;
    output[9..HEADER].copy_from_slice(&(raw.len() as u64).to_le_bytes());
    checkpoint(raw.len())?;
    output[HEADER..].copy_from_slice(raw);
    checkpoint(0)?;
    Ok(output.len())
}

pub(crate) fn decompress_dictionary_into(
    stored: &[u8],
    dictionary: &[u8],
    decoded_len: usize,
    output: &mut [u8],
    mut checkpoint: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    checkpoint(0)?;
    let (length, digest) = dictionary_frame_metadata(stored, Some(decoded_len), decoded_len)?;
    if dictionary.is_empty()
        || dictionary.len() > DICTIONARY_BYTES
        || output.len() != add(add(length, dictionary.len())?, 1)?
    {
        return Err(Error::Invalid("dictionary decoder admission"));
    }
    checkpoint(dictionary.len())?;
    if tos_foundation::Digest256::of_bytes(dictionary) != digest {
        return Err(Error::Invalid("dictionary decoder digest"));
    }
    checkpoint(dictionary.len())?;
    output[..dictionary.len()].copy_from_slice(dictionary);
    let mut decoder = DecompressorOxide::new();
    let input = &stored[DICTIONARY_HEADER..];
    let mut input_at = 0;
    let mut output_at = dictionary.len();
    let output_cap = output.len();
    loop {
        checkpoint(0)?;
        let input_end = input.len().min(add(input_at, CHUNK)?);
        let output_end = output_cap.min(add(output_at, CHUNK)?);
        if output_at == output_cap {
            return Err(Error::Invalid("dictionary decoder excess output"));
        }
        checkpoint(input_end - input_at + output_end - output_at)?;
        let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | if input_end < input.len() {
                inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
            } else {
                0
            };
        let (status, used, made) = decompress(
            &mut decoder,
            &input[input_at..input_end],
            &mut output[..output_end],
            output_at,
            flags,
        );
        if used > input_end - input_at || made > output_end - output_at {
            return Err(Error::Invalid("dictionary decoder counts"));
        }
        input_at += used;
        output_at += made;
        checkpoint(0)?;
        match status {
            TINFLStatus::Done => {
                if input_at != input.len() || output_at != dictionary.len() + decoded_len {
                    return Err(Error::Invalid("dictionary decoder exact EOF"));
                }
                return Ok(());
            }
            TINFLStatus::NeedsMoreInput | TINFLStatus::HasMoreOutput => {}
            _ => return Err(Error::Invalid("dictionary decoder stream")),
        }
        if used == 0 && made == 0 {
            return Err(Error::Invalid("dictionary decoder stalled"));
        }
    }
}

pub(crate) fn with_dictionary_decoded<T>(
    state: &CreationState<'_>,
    stored: &[u8],
    dictionary: &[u8],
    expected: Option<usize>,
    max_bytes: usize,
    consume: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    let (length, _) = dictionary_frame_metadata(stored, expected, max_bytes)?;
    let cap = add(add(length, dictionary.len())?, 1)?;
    let _hold = state.hold(add(
        add(cap, decoder_workspace_upper()?)?,
        std::mem::size_of_val(&consume),
    )?)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(cap)
        .map_err(|_| Error::Budget("dictionary decoder allocation"))?;
    if output.capacity() != cap {
        return Err(Error::Budget("dictionary decoder capacity"));
    }
    state.charge_work(cap)?;
    output.resize(cap, 0);
    decompress_dictionary_into(stored, dictionary, length, &mut output, |bytes| {
        state.active()?;
        state.charge_work(bytes)
    })?;
    consume(&output[dictionary.len()..dictionary.len() + length])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(raw: &[u8]) -> Vec<u8> {
        let mut stored = vec![0; stored_bound(raw.len()).unwrap()];
        let len = encode_into(raw, &mut stored, |_| Ok(())).unwrap();
        stored.truncate(len);
        stored
    }

    #[test]
    fn dictionary_frames_preserve_exact_bytes_and_reject_wrong_context() {
        let dictionary=b"{\"source_record\":{\"payload\":null},\"spine\":{\"id\":\"example\",\"label\":\"original Unicode \xD0\xAF\"}}".repeat(32);
        assert!(dictionary.len() <= DICTIONARY_BYTES);
        let raw = [dictionary.as_slice(), b" exact suffix\0 unchanged spelling"].concat();
        let mut stored = vec![0; stored_bound(raw.len()).unwrap()];
        let used = encode_dictionary_into(&raw, &dictionary, &mut stored, |_| Ok(())).unwrap();
        stored.truncate(used);
        assert!(is_dictionary_frame(&stored));
        assert!(frame_metadata(&stored, Some(raw.len()), raw.len()).is_err());
        let mut output = vec![0; raw.len() + dictionary.len() + 1];
        decompress_dictionary_into(&stored, &dictionary, raw.len(), &mut output, |_| Ok(()))
            .unwrap();
        assert_eq!(
            &output[dictionary.len()..dictionary.len() + raw.len()],
            &raw
        );
        let mut wrong = dictionary.clone();
        wrong[0] ^= 1;
        assert!(
            decompress_dictionary_into(&stored, &wrong, raw.len(), &mut output, |_| Ok(()))
                .is_err()
        );
        for broken in [
            stored[..stored.len() - 1].to_vec(),
            [stored.as_slice(), &[0]].concat(),
        ] {
            assert!(
                decompress_dictionary_into(
                    &broken,
                    &dictionary,
                    raw.len(),
                    &mut output,
                    |_| Ok(())
                )
                .is_err()
            );
        }
        assert!(
            decompress_dictionary_into(&stored, &dictionary, raw.len(), &mut output, |_| Err(
                Error::Budget("test caller stopped")
            ))
            .is_err()
        );
        let short = b"x";
        let mut stored = vec![0; stored_bound(short.len()).unwrap()];
        let used = encode_dictionary_into(short, &dictionary, &mut stored, |_| Ok(())).unwrap();
        assert_eq!(
            frame_metadata(&stored[..used], Some(1), 1).unwrap(),
            (1, false)
        );
        assert_eq!(&stored[HEADER..used], short);
    }

    #[test]
    fn framed_compressed_and_raw_bytes_restore_exactly() {
        let repeated =
            b"{\"source\":\"exact original spelling and Unicode \xD0\xAF\"}\n".repeat(4000);
        let mut seed = 0x0123456789abcdefu64;
        let random: Vec<u8> = (0..65536)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed as u8
            })
            .collect();
        for (raw, compressed) in [(&repeated, true), (&random, false)] {
            let stored = encode(raw);
            assert_eq!(
                frame_metadata(&stored, Some(raw.len()), raw.len()).unwrap(),
                (raw.len(), compressed)
            );
            let got = crate::knowledge_original_rows::decode_packet(
                &stored,
                raw.len(),
                raw.len(),
                raw.len() + 1 + decoder_workspace_upper().unwrap(),
                crate::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV2,
                &mut 0,
                crate::knowledge_original_rows::page_decode_work_limit(raw.len() as u64).unwrap(),
            )
            .unwrap();
            assert_eq!(&got, raw);
        }
    }

    #[test]
    fn compressed_decoder_requires_exact_length_and_stream_eof() {
        let raw = b"source bytes".repeat(2048);
        let stored = encode(&raw);
        assert!(frame_metadata(&stored, Some(raw.len() + 1), raw.len() + 1).is_err());
        for mut broken in [
            stored[..stored.len() - 1].to_vec(),
            {
                let mut v = stored.clone();
                v.push(0);
                v
            },
            stored.clone(),
        ] {
            if broken.len() == stored.len() {
                broken[9..HEADER].copy_from_slice(&((raw.len() - 1) as u64).to_le_bytes());
            }
            let declared = u64::from_le_bytes(broken[9..HEADER].try_into().unwrap()) as usize;
            assert!(
                decompress_into(&broken, declared, &mut vec![0; declared + 1], |_| Ok(())).is_err()
            );
        }
    }

    #[test]
    fn byte_codec_preserves_caller_refusal_and_format_boundary() {
        let raw = b"bounded source".repeat(4096);
        let stored = encode(&raw);
        let refused = || Error::Budget("test caller stopped");
        assert!(
            encode_into(&raw, &mut vec![0; stored_bound(raw.len()).unwrap()], |_| {
                Err(refused())
            })
            .is_err()
        );
        assert!(
            decompress_into(&stored, raw.len(), &mut vec![0; raw.len() + 1], |_| Err(
                refused()
            ))
            .is_err()
        );
        assert!(frame_metadata(&raw, None, raw.len()).is_err());
        let available = raw.len();
        assert!(
            crate::knowledge_original_rows::decode_packet(
                &stored,
                raw.len(),
                raw.len(),
                available,
                crate::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV2,
                &mut 0,
                u64::MAX,
            )
            .is_err()
        );
    }
}
