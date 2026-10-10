use pixelflasher_rust::{config::Config, services};
#[test]
fn google_acknowledgement_is_explicit_and_scoped_to_catalog() {
    assert!(!Config::default().google_terms_accepted);
    for ota in [true, false] {
        assert!(services::google_catalog_cookie(ota, false).is_err());
    }
    assert_eq!(
        services::google_catalog_cookie(false, true).unwrap(),
        "devsite_wall_acks=nexus-image-tos"
    );
    assert_eq!(
        services::google_catalog_cookie(true, true).unwrap(),
        "devsite_wall_acks=nexus-ota-tos"
    );
    let old: Config = serde_json::from_str("{\"dark\":true}").unwrap();
    assert!(!old.google_terms_accepted);
}
#[test]
fn live_google_factory_and_ota_rows_keep_their_checksums() {
    for (html, expected) in [
        (
            include_str!("fixtures/google-images-row.html"),
            "fe27e57294fbd0267b97615527613174bf45dcc0acb58825a05b5a914afd7048",
        ),
        (
            include_str!("fixtures/google-ota-row.html"),
            "7f02515d03c4a89b669c8ba537bf307ac080500f8783341a06edc4a1d1a402eb",
        ),
    ] {
        let items = services::parse_google_images(html);
        assert_eq!(items.len(), 1);
        assert!(
            items[0]
                .url
                .starts_with("https://dl.google.com/dl/android/aosp/")
        );
        assert_eq!(items[0].sha256.as_deref(), Some(expected));
    }
}
#[test]
fn catalog_without_acknowledgement_fails_before_network() {
    for ota in [true, false] {
        assert!(
            services::firmware_catalog(ota, false)
                .unwrap_err()
                .to_string()
                .contains("acknowledge")
        );
    }
}
