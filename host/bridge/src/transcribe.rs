//! Audio transcription, and folding transcripts into the knowledge graph.
//!
//! Runs entirely locally: `ffmpeg` normalises any media to 16 kHz mono WAV,
//! `whisper-cli` transcribes it, and the result is stored as a searchable
//! document alongside the workspace index. Nothing is uploaded.
//!
//! This is the transcribe-and-analyse half of the "pocket AI" idea. Capturing
//! system audio is a separate problem (macOS needs a virtual device or
//! ScreenCaptureKit); everything here works on a file that already exists.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    /// Source media path.
    pub source: String,
    /// Display title: the file stem unless the text suggests better.
    pub title: String,
    /// Full transcript text.
    pub text: String,
    /// Seconds of audio, when ffprobe could tell us.
    pub seconds: f64,
    pub words: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub items: Vec<Transcript>,
}

pub fn store_path() -> PathBuf {
    crate::paths::env_or_knowledge("OS_TRANSCRIPT_STORE", "transcripts.json")
}

/// Whisper model to use. `small` is the speed/quality compromise the
/// video-analyzer skill already settled on; `medium` is also present.
fn model_path() -> PathBuf {
    if let Ok(p) = env::var("OS_WHISPER_MODEL") {
        return PathBuf::from(p);
    }
    let dir = crate::paths::home().join(".cache/whisper-models");
    let small = dir.join("ggml-small.bin");
    if small.is_file() { small } else { dir.join("ggml-medium.bin") }
}

/// Media extensions we will accept.
const MEDIA_EXTS: &[&str] = &[
    "wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "aiff",
    "mp4", "mov", "mkv", "webm", "avi",
];

pub fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MEDIA_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Media duration in seconds, best effort.
fn duration_secs(path: &Path) -> f64 {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(path)
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Transcribe `path`. Returns the transcript without storing it.
pub fn transcribe(path: &Path) -> Result<Transcript, String> {
    if !path.is_file() {
        return Err(format!("no such file: {}", path.display()));
    }
    if !is_media(path) {
        return Err("not an audio or video file".into());
    }
    let model = model_path();
    if !model.is_file() {
        return Err(format!(
            "whisper model missing at {} (see ~/.cache/whisper-models)",
            model.display()
        ));
    }

    // Work in a temp dir so a half-finished run leaves nothing behind.
    let work = env::temp_dir().join(format!("os-transcribe-{}", std::process::id()));
    fs::create_dir_all(&work).map_err(|e| format!("mkdir: {e}"))?;
    let wav = work.join("audio.wav");

    // 16 kHz mono PCM is what whisper.cpp expects; anything else is resampled
    // internally at best and rejected at worst.
    let conv = Command::new("ffmpeg")
        .args(["-nostdin", "-y", "-i"])
        .arg(path)
        .args(["-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(&wav)
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !conv.status.success() {
        let _ = fs::remove_dir_all(&work);
        return Err(format!(
            "ffmpeg failed: {}",
            crate::text::stderr_brief(&conv.stderr, "unknown", 100)
        ));
    }

    let out = Command::new("whisper-cli")
        .arg("-m")
        .arg(&model)
        .arg("-f")
        .arg(&wav)
        .args(["-nt", "-np"]) // no timestamps, no progress noise
        .output()
        .map_err(|e| format!("whisper-cli: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let _ = fs::remove_dir_all(&work);

    if !out.status.success() {
        return Err(format!(
            "whisper failed: {}",
            crate::text::stderr_brief(&out.stderr, "unknown", 100)
        ));
    }
    if text.is_empty() {
        return Err("no speech detected".into());
    }

    Ok(Transcript {
        source: path.to_string_lossy().to_string(),
        title: title_for(path, &text),
        words: text.split_whitespace().count(),
        seconds: duration_secs(path),
        text,
    })
}

/// A readable title: the first clause of speech, falling back to the filename.
fn title_for(path: &Path, text: &str) -> String {
    let first: String = text
        .split_whitespace()
        .take(9)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(70)
        .collect();
    if first.len() >= 12 {
        first
    } else {
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("transcript").to_string()
    }
}

impl Store {
    pub fn load() -> Self {
        crate::paths::read_json_or_default(store_path())
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        crate::paths::write_json(store_path(), self)
    }

    /// Add or replace by source path, so re-transcribing updates in place.
    pub fn upsert(&mut self, t: Transcript) {
        match self.items.iter_mut().find(|i| i.source == t.source) {
            Some(existing) => *existing = t,
            None => self.items.push(t),
        }
    }
}

/// Simple extractive summary: the longest sentences carry the most content.
///
/// Deliberately not a model call — this runs on the bridge with no network and
/// no inference, and its job is to give the guest something readable in three
/// lines rather than to be clever.
pub fn summarize(text: &str, max_lines: usize) -> Vec<String> {
    let mut sentences: Vec<&str> = text
        .split(|c| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim())
        .filter(|s| s.split_whitespace().count() >= 5)
        .collect();
    sentences.sort_by_key(|s| core::cmp::Reverse(s.split_whitespace().count()));
    sentences
        .into_iter()
        .take(max_lines)
        .map(|s| s.chars().take(110).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_audio_and_video_but_not_documents() {
        assert!(is_media(Path::new("a.wav")));
        assert!(is_media(Path::new("a.MP4")), "extension match must be case-insensitive");
        assert!(is_media(Path::new("a.m4a")));
        assert!(!is_media(Path::new("a.md")));
        assert!(!is_media(Path::new("a")));
    }

    #[test]
    fn missing_file_is_reported_clearly() {
        let e = transcribe(Path::new("/nonexistent/os-audio/x.wav")).unwrap_err();
        assert!(e.contains("no such file"), "{e}");
    }

    #[test]
    fn a_document_is_refused_before_spawning_anything() {
        let p = env::temp_dir().join(format!("os-tr-{}.md", std::process::id()));
        fs::write(&p, "not audio").unwrap();
        let e = transcribe(&p).unwrap_err();
        assert!(e.contains("not an audio"), "{e}");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn title_prefers_speech_over_filename() {
        let t = title_for(Path::new("/x/rec01.wav"), "the quarterly review meeting began with revenue");
        assert!(t.starts_with("the quarterly review"), "{t}");
    }

    #[test]
    fn title_falls_back_when_speech_is_too_short() {
        assert_eq!(title_for(Path::new("/x/rec01.wav"), "ok"), "rec01");
    }

    #[test]
    fn summary_picks_substantial_sentences() {
        let text = "Hi. This is a much longer sentence carrying the actual content of the recording. Bye.";
        let s = summarize(text, 2);
        assert_eq!(s.len(), 1, "short fragments should be dropped");
        assert!(s[0].contains("actual content"));
    }

    #[test]
    fn summary_is_bounded() {
        let text = "word ".repeat(400) + ".";
        let s = summarize(&text, 3);
        assert!(s.len() <= 3);
        assert!(s.iter().all(|l| l.chars().count() <= 110));
    }

    #[test]
    fn upsert_replaces_rather_than_duplicating() {
        let mut st = Store::default();
        let mk = |w: usize| Transcript {
            source: "/a.wav".into(),
            title: "t".into(),
            text: "x".into(),
            seconds: 1.0,
            words: w,
        };
        st.upsert(mk(10));
        st.upsert(mk(20));
        assert_eq!(st.items.len(), 1, "re-transcribing must not duplicate");
        assert_eq!(st.items[0].words, 20);
    }

    #[test]
    fn store_lives_outside_the_repo() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_TRANSCRIPT_STORE") };
        let p = store_path().to_string_lossy().to_string();
        assert!(!p.contains("/os/search"), "transcripts must not land in the repo: {p}");
    }

    #[test]
    fn model_resolves_to_an_existing_cache_entry_or_is_overridable() {
        unsafe { env::set_var("OS_WHISPER_MODEL", "/tmp/custom.bin") };
        assert_eq!(model_path(), PathBuf::from("/tmp/custom.bin"));
        unsafe { env::remove_var("OS_WHISPER_MODEL") };
        assert!(model_path().to_string_lossy().contains("whisper-models"));
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn transcripts_need_their_own_grant_not_the_file_one() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-audio-scope-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        unsafe { env::set_var("OS_TRANSCRIPT_STORE", dir.join("t.json")) };

        let mut st = Store::default();
        // Unique tokens only — common words like "audio" drown in teddysearch.
        st.upsert(Transcript {
            source: "/tmp/call.wav".into(),
            title: "ZygoteNotary Briefing".into(),
            text: "the zygotenotary briefing covered settlement".into(),
            seconds: 10.0,
            words: 5,
        });
        st.save().unwrap();

        // workspace.index granted, audio.transcribe not: must stay hidden.
        let files_only =
            crate::search::query_all("zygotenotary", 5, None, false, true, false);
        assert!(
            !files_only.iter().any(|r| r.contains("ZygoteNotary")),
            "a recording surfaced under the files grant: {files_only:?}"
        );

        let with_audio =
            crate::search::query_all("zygotenotary", 5, None, false, false, true);
        assert!(
            with_audio.iter().any(|r| r.contains("ZygoteNotary")),
            "granted search should find the transcript: {with_audio:?}"
        );

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_TRANSCRIPT_STORE") };
    }
}
