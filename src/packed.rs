use serde_json::{Value, json};

fn firmware(raw: &str) -> Option<(Value, bool)> {
    if raw.len() < 44 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&raw[i..i + 2], 16).ok();
    let date = chrono::NaiveDate::from_ymd_opt(
        i32::from(byte(12)?) * 100 + i32::from(byte(14)?),
        u32::from(byte(16)?),
        u32::from(byte(18)?),
    )
    .and_then(|d| {
        d.and_hms_opt(
            u32::from(byte(20)?),
            u32::from(byte(22)?),
            u32::from(byte(24)?),
        )
    });
    let version =
        |i: usize| -> Option<String> { Some(format!("{}.{:02}", byte(i)?, byte(i + 2)?)) };
    let locale: Vec<_> = (2..12).step_by(2).map(byte).collect::<Option<Vec<_>>>()?;
    let locale_text = if locale.iter().all(|b| *b == 0) {
        None
    } else {
        String::from_utf8(locale).ok()
    };
    let rollback = raw.get(44..46);
    let condition = match &raw[..2] {
        "00" => Some("no_update_ready"),
        "01" => Some("update_ready"),
        "02" => Some("update_complete"),
        _ => None,
    };
    let rollback_flag = match rollback {
        Some("00") => Some(false),
        Some("01") => Some(true),
        _ => None,
    };
    Some((
        json!({
            "ready_condition_raw":&raw[..2],"ready_condition":condition,
            "locale_raw":&raw[2..12],"locale":locale_text,
            "date_components":(12..26).step_by(2).map(byte).collect::<Vec<_>>(),
            "instructed_update_utc":date.map(|d|format!("{}Z",d.format("%Y-%m-%dT%H:%M:%S"))),
            "current_version_raw":&raw[26..32],"current_version":version(26),
            "current_version_suffix_ascii":byte(30).filter(|b|(32..127).contains(b)).map(char::from),
            "previous_version_raw":&raw[32..38],"previous_version":if &raw[32..38]=="000000" {None} else {version(32)},
            "update_version_raw":&raw[38..44],"update_version":if &raw[38..44]=="000000" {None} else {version(38)},
            "rollback_raw":rollback,"rollback":rollback_flag,"uninterpreted_tail":raw.get(46..).unwrap_or("")
        }),
        raw.len() != 46 || condition.is_none() || rollback_flag.is_none(),
    ))
}

pub fn decode(id: &str, raw: &str) -> Option<(Value, bool)> {
    if !raw.is_ascii() || raw.len() % 2 != 0 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |offset: usize| u8::from_str_radix(&raw[offset..offset + 2], 16).ok();
    if id == "012C" && raw.len() == 26 {
        let year = 2000 + i32::from(byte(8)?);
        let date =
            chrono::NaiveDate::from_ymd_opt(year, u32::from(byte(10)?), u32::from(byte(12)?))?;
        let time = date.and_hms_opt(u32::from(byte(14)?), u32::from(byte(16)?), 0)?;
        if byte(20)? >= 60 {
            return None;
        }
        return Some((
            json!({
                "course_code": &raw[..2], "subcourse_code": &raw[2..8],
                "operation_end_local": format!("{}+09:00", time.format("%Y-%m-%dT%H:%M:%S")),
                "operation_duration_minutes": u32::from(byte(18)?)*60+u32::from(byte(20)?),
                "uninterpreted_tail": &raw[22..],
                "evidence": "Field boundaries inferred from one matching official laundry-report notification; other layouts and tail semantics remain unverified."
            }),
            true,
        ));
    }
    if id == "00F0" && raw.len() == 2 {
        let mode = match raw {
            "00" => "unspecified",
            "01" => "pre_cleaning",
            "02" => "sterile_deodorant_tank_rolling",
            "03" => "sterile_deodorant_tank_stop",
            "04" => "pollen",
            "05" => "virus_care",
            _ => return None,
        };
        return Some((json!(mode), false));
    }
    if id == "01E0" && raw.len() >= 90 {
        return Some((
            json!({"course_slots": (0..90).step_by(6).map(|i| &raw[i..i+6]).collect::<Vec<_>>(), "uninterpreted_tail": &raw[90..]}),
            true,
        ));
    }
    let full = match id {
        "00B2" if raw == "43" => json!("停止"),
        "0092" | "00F2" if raw.len() == 4 && byte(2)? < 60 => {
            json!({"hours":byte(0)?, "minutes":byte(2)?, "total_minutes":u32::from(byte(0)?)*60+u32::from(byte(2)?)})
        }
        "00E7" if raw.len() == 8 => {
            let bits = u32::from_str_radix(raw, 16).ok()?;
            json!((0..8).map(|i| {
                let code = (bits >> (4*i)) & 15;
                json!({"rinse":i+1,"code":code,"mode":match code {
                    0=>Some("unspecified"),1=>Some("stored_water"),2=>Some("flowing_water"),3=>Some("shower"),_=>None
                }})
            }).collect::<Vec<_>>())
        }
        "012B" if raw.len() % 8 == 0 => json!((0..raw.len()).step_by(8).filter_map(|i| {
            let slot=&raw[i..i+8];
            if &slot[..2]=="00" { return None; }
            Some(if ["4B","8B","CB","BE"].contains(&&slot[..2]) {
                json!({"type":&slot[..2],"course":&slot[2..],"raw":slot})
            } else {json!({"course":&slot[..2],"subcourse":&slot[2..4],"uninterpreted_tail":&slot[4..],"raw":slot})})
        }).collect::<Vec<_>>()),
        "01E0" | "01E2" if raw.len() % 6 == 0 => json!((0..raw.len()).step_by(6).map(|i| &raw[i..i+6]).collect::<Vec<_>>()),
        "00FA" if raw.len() == 6 => json!({"extended_course_code":raw}),
        "01E6" if raw.len() == 8 => json!({"slot":byte(0)?,"course_code":&raw[2..]}),
        "0086" if raw.len() == 4 => json!({"kind":match byte(0)? {0=>Some("none"),1=>Some("U"),2=>Some("H"),3=>Some("F"),_=>None},"number":byte(2)?}),
        "013F" if raw.len() >= 14 => return Some((json!({
            "wash_dry_wash_time_raw":&raw[..4],"wash_dry_rinse_count_raw":&raw[4..6],
            "dry_setting_raw":&raw[6..8],"wash_only_wash_time_raw":&raw[8..12],
            "wash_only_rinse_count_raw":&raw[12..14],"uninterpreted_tail":&raw[14..]
        }),true)),
        "0150" if raw.len()==6 => return Some((json!({"rinse_count":if byte(4)? & 64 != 0 {1} else {2},"uninterpreted_bits":format!("{:06X}",u32::from_str_radix(raw,16).ok()? & !64)}),true)),
        "0152" if raw.len()==4 && byte(0)?<=1 => return Some((json!({"ai_enabled":byte(0)?==1,"uninterpreted_tail":&raw[2..]}),true)),
        "01F0" => return firmware(raw),
        _=>return None
    };
    Some((full, id == "012B"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn washing_report_matches_independent_notification_fields() {
        let (v, partial) = decode("012C", "210000001A0A09113402090004").unwrap();
        assert!(partial);
        assert_eq!(v["course_code"], "21");
        assert_eq!(v["subcourse_code"], "000000");
        assert_eq!(v["operation_end_local"], "2026-10-09T17:52:00+09:00");
        assert_eq!(v["operation_duration_minutes"], 129);
        assert_eq!(v["uninterpreted_tail"], "0004");
        assert!(decode("012C", "210000001AFF09113402090004").is_none());
    }
    #[test]
    fn firmware_truncation_and_reserved_fields_remain_visible() {
        let (v, partial) = decode("01F0", "00000000000000000000000000010030000000000000").unwrap();
        assert!(partial);
        assert_eq!(v["rollback_raw"], Value::Null);
        assert!(decode("01F0", "0000").is_none());
        let (v, _) = decode("00E7", "000000F1").unwrap();
        assert_eq!(v[0]["mode"], "stored_water");
        assert_eq!(v[1]["mode"], Value::Null);
    }
    #[test]
    fn firmware_decodes_version_suffix_utc_and_rollback_from_app_layout() {
        let (v, partial) =
            decode("01F0", "016A615F4A50141A0A0911340201003000000001023101").unwrap();
        assert!(!partial);
        assert_eq!(v["locale"], "ja_JP");
        assert_eq!(v["current_version"], "1.00");
        assert_eq!(v["current_version_suffix_ascii"], "0");
        assert_eq!(v["instructed_update_utc"], "2026-10-09T17:52:02Z");
        assert_eq!(v["rollback"], true);
    }
}
