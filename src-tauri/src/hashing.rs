//! Stable lowercase hexadecimal encoding for persisted SHA-256 proofs.
//! Keep hashing inputs and serialization at each call site unchanged.
pub fn hex(bytes:&[u8])->String {
 const DIGITS:&[u8;16]=b"0123456789abcdef";
 let mut out=String::with_capacity(bytes.len()*2);
 for &b in bytes { out.push(DIGITS[(b>>4) as usize] as char);out.push(DIGITS[(b&15) as usize] as char); }
 out
}
#[cfg(test)]
mod tests {
 use super::*;
 use sha2::{Digest,Sha256};
 #[test]
 fn sha256_known_answers_and_production_wrappers() {
  for (input,expected) in [
   (b"".as_slice(),"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
   (b"abc".as_slice(),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
  ] {
   assert_eq!(hex(&Sha256::digest(input)),expected);
   assert_eq!(crate::purposes::sha256(input),expected);
   assert_eq!(crate::recovery::sha(input),expected);
   assert_eq!(crate::skin_package::hash(input),expected);
   let mut h=Sha256::new();for chunk in input.chunks(1) {h.update(chunk);}
   assert_eq!(hex(&h.finalize()),expected);
  }
 }
 #[test]
 fn byte_encoding_preserves_leading_zeroes_and_lowercase() {
  let bytes=(0u8..=255).collect::<Vec<_>>();
  let expected=bytes.iter().map(|b|format!("{b:02x}")).collect::<String>();
  assert_eq!(hex(&bytes),expected);assert_eq!(hex(&[0,1,15,16,255]),"00010f10ff");
 }
}
