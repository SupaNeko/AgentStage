use std::fs;

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{connection::{get_db, DbState}, sticker as sticker_repo};
use crate::models::sticker::*;

#[tauri::command]
pub async fn list_sticker_packs(state: State<'_, DbState>) -> Result<Vec<StickerPackResponse>, String> {
    let conn = get_db(&state).await?;
    sticker_repo::list_packs(&conn)
}

#[tauri::command]
pub async fn create_sticker_pack(
    state: State<'_, DbState>,
    req: CreateStickerPackRequest,
) -> Result<StickerPackResponse, String> {
    let conn = get_db(&state).await?;
    sticker_repo::create_pack(&conn, &req.name)
}

#[tauri::command]
pub async fn update_sticker_pack(
    state: State<'_, DbState>,
    req: UpdateStickerPackRequest,
) -> Result<StickerPackResponse, String> {
    let conn = get_db(&state).await?;
    sticker_repo::update_pack_name(&conn, &req.id, &req.name)
}

#[tauri::command]
pub async fn delete_sticker_pack(
    state: State<'_, DbState>,
    req: DeleteStickerPackRequest,
) -> Result<(), String> {
    let mut conn = get_db(&state).await?;
    sticker_repo::delete_pack(&mut conn, &req.id)
}

pub fn extension_from_mime(mime: &str) -> Result<&'static str, String> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        _ => Err("不支持的图片类型".to_string()),
    }
}

/// 清洗名称：将下划线替换为空格，trim 后如果为空则返回 "未命名"
pub fn sanitize_name(name: &str) -> String {
    let cleaned = name.replace('_', " ").trim().to_string();
    if cleaned.is_empty() {
        "未命名".to_string()
    } else {
        cleaned
    }
}

fn decode_data_url(data: &str) -> Result<(Vec<u8>, Option<String>), String> {
    if let Some(comma) = data.find(',') {
        let header = &data[..comma];
        let mime = header
            .strip_prefix("data:")
            .and_then(|s| s.split(';').next())
            .map(|s| s.to_string());
        let bytes = general_purpose::STANDARD.decode(&data[comma + 1..]).map_err(|e| e.to_string())?;
        return Ok((bytes, mime));
    }
    Ok((general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())?, None))
}

fn detect_mime(bytes: &[u8], hinted: Option<String>) -> Result<String, String> {
    if bytes.starts_with(b"\x89PNG") {
        return Ok("image/png".to_string());
    }
    if bytes.starts_with(b"\xff\xd8") {
        return Ok("image/jpeg".to_string());
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif".to_string());
    }
    hinted.filter(|m| m == "image/png" || m == "image/jpeg" || m == "image/gif")
        .ok_or_else(|| "不支持的图片类型".to_string())
}

fn validate_compression_ratio(ratio: f32) -> Result<f32, String> {
    if !ratio.is_finite() || ratio <= 0.0 || ratio > 1.0 {
        return Err("压缩倍率必须大于 0 且小于等于 1".to_string());
    }
    Ok(ratio)
}

fn gif_dimensions(bytes: &[u8]) -> Result<(i32, i32), String> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Err("无效的 GIF 文件".to_string());
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]) as i32;
    let height = u16::from_le_bytes([bytes[8], bytes[9]]) as i32;
    if width <= 0 || height <= 0 {
        return Err("无效的 GIF 尺寸".to_string());
    }
    Ok((width, height))
}

#[tauri::command]
pub async fn add_sticker_to_pack(
    state: State<'_, DbState>,
    req: AddStickerRequest,
) -> Result<StickerResponse, String> {
    let compression_ratio = validate_compression_ratio(req.compression_ratio)?;
    let (bytes, hinted_mime) = decode_data_url(&req.image_data_base64)?;
    let mime_type = detect_mime(&bytes, hinted_mime)?;

    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let pack_dir = data_dir.join("stickers").join(&req.pack_id);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;

    let ext = extension_from_mime(&mime_type)?;
    let sticker_id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{}.{}", sticker_id, ext);
    let file_path = pack_dir.join(&file_name);

    let (width, height, file_size) = if mime_type == "image/gif" {
        fs::write(&file_path, &bytes).map_err(|e| e.to_string())?;
        let (w, h) = gif_dimensions(&bytes)?;
        let size = fs::metadata(&file_path).map_err(|e| e.to_string())?.len() as i64;
        (w, h, size)
    } else {
        let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let (orig_w, orig_h) = (img.width() as f32, img.height() as f32);
        let processed = if compression_ratio < 1.0 {
            let new_w = (orig_w * compression_ratio).max(1.0) as u32;
            let new_h = (orig_h * compression_ratio).max(1.0) as u32;
            image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Lanczos3)
        } else {
            img.to_rgba8()
        };

        let (final_w, final_h) = (processed.width() as i32, processed.height() as i32);

        if mime_type == "image/png" {
            let file = fs::File::create(&file_path).map_err(|e| e.to_string())?;
            let encoder = image::codecs::png::PngEncoder::new(&file);
            encoder.write_image(&processed, processed.width(), processed.height(), image::ExtendedColorType::Rgba8)
                .map_err(|e| e.to_string())?;
        } else {
            let rgb_img = image::DynamicImage::ImageRgba8(processed).to_rgb8();
            let file = fs::File::create(&file_path).map_err(|e| e.to_string())?;
            let encoder = image::codecs::jpeg::JpegEncoder::new(&file);
            encoder.write_image(&rgb_img, rgb_img.width(), rgb_img.height(), image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
        }

        let size = fs::metadata(&file_path).map_err(|e| e.to_string())?.len() as i64;
        (final_w, final_h, size)
    };

    // 自动清洗表情名，避免文件名中的下划线导致失败
    let sanitized_name = sanitize_name(&req.name);
    let relative_path = format!("stickers/{}/{}", req.pack_id, file_name);
    let conn = get_db(&state).await?;
    sticker_repo::insert_sticker_metadata(&conn, &req.pack_id, &sanitized_name, &relative_path, &mime_type, width, height, file_size)
}

#[tauri::command]
pub async fn update_sticker(
    state: State<'_, DbState>,
    req: UpdateStickerRequest,
) -> Result<StickerResponse, String> {
    let conn = get_db(&state).await?;
    sticker_repo::update_sticker_name(&conn, &req.id, &req.name)
}

#[tauri::command]
pub async fn delete_stickers(
    state: State<'_, DbState>,
    req: DeleteStickersRequest,
) -> Result<(), String> {
    let mut conn = get_db(&state).await?;
    sticker_repo::delete_stickers(&mut conn, &req.ids)
}

#[tauri::command]
pub async fn list_agent_sticker_packs(
    state: State<'_, DbState>,
    req: ListAgentStickerPacksRequest,
) -> Result<Vec<String>, String> {
    let conn = get_db(&state).await?;
    sticker_repo::list_agent_pack_ids(&conn, &req.agent_id)
}

#[tauri::command]
pub async fn set_agent_sticker_packs(
    state: State<'_, DbState>,
    req: SetAgentStickerPacksRequest,
) -> Result<(), String> {
    let mut conn = get_db(&state).await?;
    sticker_repo::set_agent_pack_ids(&mut conn, &req.agent_id, &req.pack_ids)
}

#[tauri::command]
pub async fn resolve_sticker_refs(
    state: State<'_, DbState>,
    req: ResolveStickerRefsRequest,
) -> Result<Vec<ResolvedStickerResponse>, String> {
    let conn = get_db(&state).await?;
    sticker_repo::resolve_refs(&conn, &req.refs)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct StickerPackBundle {
    #[serde(alias = "format")]
    format: String,
    #[serde(alias = "version")]
    version: i32,
    #[serde(alias = "exportedAt")]
    exported_at: i64,
    #[serde(alias = "pack")]
    pack: StickerPackBundlePack,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct StickerPackBundlePack {
    #[serde(alias = "name")]
    name: String,
    #[serde(alias = "stickers")]
    stickers: Vec<StickerPackBundleSticker>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct StickerPackBundleSticker {
    #[serde(alias = "name")]
    name: String,
    #[serde(alias = "mimeType")]
    mime_type: String,
    #[serde(alias = "width")]
    width: i32,
    #[serde(alias = "height")]
    height: i32,
    #[serde(alias = "fileSize")]
    file_size: i64,
    #[serde(alias = "base64Content")]
    base64_content: String,
}

#[tauri::command]
pub async fn export_sticker_pack(
    state: State<'_, DbState>,
    req: ExportStickerPackRequest,
) -> Result<ExportStickerPackResponse, String> {
    let conn = get_db(&state).await?;
    let pack = sticker_repo::get_pack(&conn, &req.pack_id)?
        .ok_or_else(|| "表情包不存在或已删除".to_string())?;

    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let mut warnings = Vec::new();
    let mut bundle_stickers = Vec::new();

    for sticker in &pack.stickers {
        let abs_path = data_dir.join(&sticker.file_path);
        match fs::read(&abs_path) {
            Ok(bytes) => {
                bundle_stickers.push(StickerPackBundleSticker {
                    name: sticker.name.clone(),
                    mime_type: sticker.mime_type.clone(),
                    width: sticker.width,
                    height: sticker.height,
                    file_size: sticker.file_size,
                    base64_content: general_purpose::STANDARD.encode(&bytes),
                });
            }
            Err(e) => {
                warnings.push(format!("无法读取表情文件 {}: {}", sticker.name, e));
            }
        }
    }

    let bundle = StickerPackBundle {
        format: "agentstage.sticker_pack".to_string(),
        version: 1,
        exported_at: chrono::Utc::now().timestamp_millis(),
        pack: StickerPackBundlePack {
            name: pack.name.clone(),
            stickers: bundle_stickers,
        },
    };

    let export_dir = data_dir.join("exports").join("stickers");
    fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
    let file_name = format!("{}.agentsticker", pack.name);
    let file_path = export_dir.join(&file_name);
    let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    fs::write(&file_path, &json).map_err(|e| e.to_string())?;

    Ok(ExportStickerPackResponse {
        exported_path: file_path.to_string_lossy().to_string(),
        file_content: json,
        warnings,
    })
}

#[tauri::command]
pub async fn import_sticker_pack(
    state: State<'_, DbState>,
    req: ImportStickerPackRequest,
) -> Result<ImportStickerPackResponse, String> {
    let bundle: StickerPackBundle = serde_json::from_str(&req.file_content)
        .map_err(|e| format!("解析文件失败: {}", e))?;

    if bundle.format != "agentstage.sticker_pack" {
        return Err("无效的文件格式".to_string());
    }
    if bundle.version != 1 {
        return Err("不支持的文件版本".to_string());
    }

    let mut conn = get_db(&state).await?;
    let existing_packs = sticker_repo::list_packs(&conn)?;
    let existing_names: Vec<String> = existing_packs.iter().map(|p| p.name.clone()).collect();

    let pack_name = sanitize_name(&bundle.pack.name);
    let final_pack_name = sticker_repo::next_available_name(&pack_name, |n| existing_names.contains(&n.to_string()));
    let renamed = final_pack_name != bundle.pack.name;

    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let pack = sticker_repo::create_pack(&conn, &final_pack_name)?;
    let pack_dir = data_dir.join("stickers").join(&pack.id);
    fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;

    let mut warnings = Vec::new();
    let mut sticker_names: Vec<String> = Vec::new();
    let mut written_files: Vec<std::path::PathBuf> = Vec::new();

    for bundle_sticker in &bundle.pack.stickers {
        let sanitized = sanitize_name(&bundle_sticker.name);
        let final_name = sticker_repo::next_available_name(&sanitized, |n| sticker_names.contains(&n.to_string()));
        if final_name != bundle_sticker.name {
            warnings.push(format!("表情 '{}' 清洗/重命名为 '{}'", bundle_sticker.name, final_name));
        }
        sticker_names.push(final_name.clone());

        let bytes = match general_purpose::STANDARD.decode(&bundle_sticker.base64_content) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!("解码表情 {} 失败: {}", bundle_sticker.name, e));
                continue;
            }
        };

        let ext = match extension_from_mime(&bundle_sticker.mime_type) {
            Ok(e) => e,
            Err(e) => {
                warnings.push(format!("表情 {} 格式不支持: {}", bundle_sticker.name, e));
                continue;
            }
        };
        let sticker_id = uuid::Uuid::new_v4().to_string();
        let file_name = format!("{}.{}", sticker_id, ext);
        let file_path = pack_dir.join(&file_name);
        if let Err(e) = fs::write(&file_path, &bytes) {
            warnings.push(format!("写入表情 {} 文件失败: {}", final_name, e));
            continue;
        }
        written_files.push(file_path.clone());

        let relative_path = format!("stickers/{}/{}", pack.id, file_name);
        if let Err(e) = sticker_repo::insert_sticker_metadata(
            &conn,
            &pack.id,
            &final_name,
            &relative_path,
            &bundle_sticker.mime_type,
            bundle_sticker.width,
            bundle_sticker.height,
            bundle_sticker.file_size,
        ) {
            warnings.push(format!("导入表情 {} 失败: {}", final_name, e));
            // 失败时删除已写入的文件，避免残留
            let _ = fs::remove_file(&file_path);
            continue;
        }
    }

    // 如果所有表情都导入失败，且原本有贴纸，则清理空包
    let pack = match sticker_repo::get_pack(&conn, &pack.id)? {
        Some(p) => p,
        None => return Err("导入后读取表情包失败".to_string()),
    };

    if pack.stickers.is_empty() && !bundle.pack.stickers.is_empty() {
        // 全部失败，清理空包
        let _ = sticker_repo::delete_pack(&mut *conn, &pack.id);
        let _ = fs::remove_dir_all(&pack_dir);
        return Err("所有表情导入失败，请检查文件内容".to_string());
    }

    Ok(ImportStickerPackResponse {
        pack,
        renamed,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn extension_from_mime_supports_expected_types() {
        assert_eq!(super::extension_from_mime("image/png").unwrap(), "png");
        assert_eq!(super::extension_from_mime("image/jpeg").unwrap(), "jpg");
        assert_eq!(super::extension_from_mime("image/gif").unwrap(), "gif");
        assert!(super::extension_from_mime("image/webp").is_err());
    }
}
