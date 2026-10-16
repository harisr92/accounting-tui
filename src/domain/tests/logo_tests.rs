use crate::domain::{DomainError, Logo, LogoFormat, LogoId, MAX_LOGO_BYTES};

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const JPEG_HEADER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];

#[test]
fn accepts_png_and_jpeg() {
    let png = Logo::new(PNG_HEADER.to_vec()).unwrap();
    assert_eq!(png.format(), LogoFormat::Png);
    assert_eq!(png.bytes(), PNG_HEADER);
    let jpeg = Logo::new(JPEG_HEADER.to_vec()).unwrap();
    assert_eq!(jpeg.format(), LogoFormat::Jpeg);
}

#[test]
fn every_new_logo_gets_its_own_id() {
    let first = Logo::new(PNG_HEADER.to_vec()).unwrap();
    let second = Logo::new(PNG_HEADER.to_vec()).unwrap();
    assert_ne!(first.id(), second.id());
}

#[test]
fn rejects_anything_but_an_image() {
    for bytes in [Vec::new(), b"GIF89a".to_vec(), b"<svg/>".to_vec()] {
        assert!(matches!(Logo::new(bytes), Err(DomainError::InvalidLogo)));
    }
}

#[test]
fn rejects_oversized_images() {
    let mut bytes = PNG_HEADER.to_vec();
    bytes.resize(MAX_LOGO_BYTES + 1, 0);
    assert!(matches!(
        Logo::new(bytes),
        Err(DomainError::LogoTooLarge { .. })
    ));
}

#[test]
fn stored_logos_keep_their_id_and_are_checked_again() {
    let id = LogoId::generate();
    assert_eq!(Logo::from_stored(id, PNG_HEADER.to_vec()).unwrap().id(), id);
    assert!(Logo::from_stored(id, b"junk".to_vec()).is_err());
}
