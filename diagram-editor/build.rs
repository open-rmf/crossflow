/// Builds the frontend and packages it into a tar.gz archive.
/// This requires `pnpm` and all the js dependencies to be available.
///
/// This build script is excluded from the crate, the output tarball is included instead.
/// This allows downstream to build the crate without any of the js stack.

#[cfg(feature = "frontend")]
mod frontend {
    use flate2::{Compression, write::GzEncoder};
    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tar::Builder;
    use sha2::Digest;

    fn build_pnpm<W: std::io::Write>(make_encoding: impl FnOnce() -> W) {
        let status = Command::new("pnpm")
            .arg("install")
            .arg("--frozen-lockfile")
            .status()
            .expect("Failed to execute pnpm install");

        if !status.success() {
            panic!("pnpm install failed with status: {:?}", status);
        }

        let status = Command::new("pnpm")
            .arg("build")
            .status()
            .expect("Failed to execute pnpm build");

        if !status.success() {
            panic!("pnpm build failed with status: {:?}", status);
        }

        let dist_dir_path = "dist";

        if Path::new(dist_dir_path).exists() {
            let mut tar_builder = Builder::new(make_encoding());
            tar_builder.mode(tar::HeaderMode::Deterministic);

            let mut paths_to_archive: Vec<PathBuf> = Vec::new();
            for entry in walkdir::WalkDir::new(dist_dir_path) {
                let entry = entry.expect("Failed to read directory entry");
                let path = entry.path();
                if path.is_file() {
                    paths_to_archive.push(path.to_path_buf());
                }
            }
            paths_to_archive.sort_unstable();

            for path in paths_to_archive {
                let relative_path = path
                    .strip_prefix(dist_dir_path)
                    .expect("Failed to strip prefix");
                tar_builder
                    .append_path_with_name(&path, relative_path)
                    .expect("Failed to add file to tar archive");
            }
            tar_builder.finish().expect("Failed to finish tar archive");
            println!("Successfully compressed '{}'", dist_dir_path,);
        } else {
            panic!(
                "Directory '{}' not found after pnpm build. Frontend build might have failed.",
                dist_dir_path
            );
        }
    }

    /// Used to save dist.tar.gz, a compressed version of the built frontend,
    /// for embedding into an executor binary so it can be used by downstream
    /// crates without needing any locally installed pnpm machinery.
    fn build_compressed_pnpm<P: AsRef<Path>>(dist_tar_gz_path: P) {
        build_pnpm(move || {
            let dist_tar_gz_file = File::create(dist_tar_gz_path)
                .expect("Failed to create output file for dist.tar.gz");
            GzEncoder::new(dist_tar_gz_file, Compression::default())
        });
    }

    /// Use to create an uncompressed version of the built frontend to check it
    /// against the cached blob. This allows us to detect changes to the frontend
    /// that would require us to recompile the crossflow-diagram-editor. We test
    /// against an uncompressed version to avoid the problem reported here:
    /// https://github.com/open-rmf/crossflow/issues/233
    fn build_uncompressed_pnpm<P: AsRef<Path>>(dist_tar_path: P) {
        build_pnpm(move || {
            File::create(dist_tar_path).expect("Failed to create output file for dist.tar")
        });
    }

    fn create_dist_hash<P: AsRef<Path>>(dist_tar_path: P) -> String {
        let dist_tar_file = File::open(dist_tar_path).expect("dist.tar not found");
        let mut reader = BufReader::new(dist_tar_file);
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut reader, &mut hasher).expect("failed to pass dist.tar through hasher");
        let hash = hasher.finalize();
        format!("{:x}", hash)
    }

    fn verify_hashes(
        mut cached_dist_hash_file: File,
        expected_hash: &str,
    ) -> bool {
        let mut actual_hash = String::new();
        if cached_dist_hash_file.read_to_string(&mut actual_hash).is_err() {
            return false;
        }

        actual_hash == expected_hash
    }

    pub fn build_frontend() {
        println!("cargo::rerun-if-changed=package.json");
        println!("cargo::rerun-if-changed=../pnpm-lock.yaml");
        println!("cargo::rerun-if-changed=rsbuild.config.ts");
        println!("cargo::rerun-if-changed=frontend");

        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

        let dist_tar_path = Path::new(&out_dir).join("dist.tar");
        build_uncompressed_pnpm(&dist_tar_path);

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set");
        let cached_dist_hash_path = Path::new(&manifest_dir).join("dist.hash");
        let cached_dist_hash_file = File::open(&cached_dist_hash_path);

        let dist_tar_gz_path = Path::new(&out_dir).join("dist.tar.gz");
        let dist_tar_gz_exists = std::fs::exists(&dist_tar_gz_path).is_ok_and(|x| x);

        let dist_hash = create_dist_hash(dist_tar_path);

        if dist_tar_gz_exists && let Ok(cached_dist_hash_file) = cached_dist_hash_file {
            // We have a compressed frontend build available and hash file saved.
            // Now just check if the saved hash matches the latest uncompressed hash.
            if verify_hashes(cached_dist_hash_file, &dist_hash) {
                return;
            }
        }

        // The dist.tar.gz doesn't exist already, or the hash doesn't exist to
        // verify it.
        println!("cargo::warning=Rebuilding frontend server after a change in the frontend UI");
        build_compressed_pnpm(&dist_tar_gz_path);
        std::fs::write(cached_dist_hash_path, &dist_hash).expect("Failed to save dist.hash");
        println!("cargo::rerun-if-changed=dist.hash");
    }
}

fn main() {
    #[cfg(feature = "frontend")]
    {
        frontend::build_frontend();
    }
}
