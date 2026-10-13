//! Stamps the commit this binary was built from (benilla's shared build-script body), including
//! the dev build's project folder, where the `WoW` junction lives.

fn main() {
    benilla_buildstamp::emit();
}
