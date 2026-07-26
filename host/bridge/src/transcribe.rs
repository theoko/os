//! Audio transcription, and folding transcripts into searchable documents.
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Transcript {
    /// Source media path.
    pub(crate) source: String,
    /// Display title: the file stem unless the text suggests better.
    pub(crate) title: String,
    /// Full transcript text.
    pub(crate) text: String,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Store {
    #[serde(default)]
    pub(crate) items: Vec<Transcript>,
}

pub(crate) fn store_path() -> PathBuf {
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

fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MEDIA_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Transcribe `path`. Returns the transcript without storing.
pub(crate) fn transcribe(path: &Path) -> Result<Transcript, String> {
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
        text,
    })
}

/// A readable title: the first clause of speech, falling back to the filename.
fn title_for(path: &Path, text: &str) -> String {
    let mut first = String::new();
    for (i, w) in text.split_whitespace().take(9).enumerate() {
        if i > 0 {
            first.push(' ');
        }
        first.push_str(w);
    }
    if first.chars().count() > 70 {
        first = first.chars().take(70).collect();
    }
    if first.len() >= 12 {
        first
    } else {
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("transcript").to_string()
    }
}

impl Store {
    pub(crate) fn load() -> Self {
        crate::paths::read_json_or_default(store_path())
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        crate::paths::write_json(store_path(), self)
    }

    /// Add or replace by source path, so re-transcribing updates in place.
    pub(crate) fn upsert(&mut self, t: Transcript) {
        match self.items.iter_mut().find(|i| i.source == t.source) {
            Some(existing) => *existing = t,
            None => self.items.push(t),
        }
    }
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
    fn title_prefers_speech_else_falls_back_to_filename() {
        let t = title_for(Path::new("/x/rec01.wav"), "the quarterly review meeting began with revenue");
        assert!(t.starts_with("the quarterly review"), "{t}");
        assert_eq!(title_for(Path::new("/x/rec01.wav"), "ok"), "rec01");
    }

    #[test]
    fn upsert_replaces_rather_than_duplicating() {
        let mut st = Store::default();
        let mk = |text: &str| Transcript {
            source: "/a.wav".into(),
            title: "t".into(),
            text: text.into(),
        };
        st.upsert(mk("first"));
        st.upsert(mk("second"));
        assert_eq!(st.items.len(), 1, "re-transcribing must not duplicate");
        assert_eq!(st.items[0].text, "second");
    }

    #[test]
    fn store_lives_outside_the_repo() {
        let _g = crate::paths::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        let _g = crate::paths::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        });
        st.save().unwrap();

        // workspace.index granted, audio.transcribe not: must stay hidden.
        let files_only =
            crate::search::query_all("zygotenotary", 5, true, false);
        assert!(
            !files_only.iter().any(|r| r.contains("ZygoteNotary")),
            "a recording surfaced under the files grant: {files_only:?}"
        );

        let with_audio =
            crate::search::query_all("zygotenotary", 5, false, true);
        assert!(
            with_audio.iter().any(|r| r.contains("ZygoteNotary")),
            "granted search should find the transcript: {with_audio:?}"
        );

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_TRANSCRIPT_STORE") };
    }
}
