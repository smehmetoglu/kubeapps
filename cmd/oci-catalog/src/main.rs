// Copyright 2023 the Kubeapps contributors.
// SPDX-License-Identifier: Apache-2.0

use self::providers::OCICatalogSender;
use clap::Parser;
use log;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

// Ensure that the compiled proto API is available within a module
// before importing the required items.
pub mod oci_catalog {
    tonic::include_proto!("ocicatalog");
}
use oci_catalog::oci_catalog_server::{OciCatalog, OciCatalogServer};
use oci_catalog::{ListRepositoriesRequest, ListTagsRequest, Repository, Tag};

mod cli;
mod providers;

#[derive(Debug, Default)]
pub struct KubeappsOCICatalog {}

#[tonic::async_trait]
impl OciCatalog for KubeappsOCICatalog {
    type ListRepositoriesForRegistryStream = ReceiverStream<Result<Repository, Status>>;
    type ListTagsForRepositoryStream = ReceiverStream<Result<Tag, Status>>;

    async fn list_repositories_for_registry(
        &self,
        request: Request<ListRepositoriesRequest>,
    ) -> Result<Response<Self::ListRepositoriesForRegistryStream>, Status> {
        // Initially for prototype, just implement support for
        // docker's registry-1.docker.io. Later split out relevant
        // functionality to a trait that can be implemented separately
        // by different services (harbor, gcr etc.)
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            // Have a trait which each registry plugin implements for matching a request.
            match request.get_ref().registry.as_str() {
                "registry-1.docker.io" => {
                    providers::dockerhub::DockerHubAPI::send_repositories(tx, request.get_ref())
                        .await;
                }
                _ => {
                    unimplemented!()
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn list_tags_for_repository(
        &self,
        request: Request<ListTagsRequest>,
    ) -> Result<Response<Self::ListTagsForRepositoryStream>, Status> {
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            // Possibly just use generic OCI API for listing tags.
            providers::dockerhub::DockerHubAPI::send_tags(tx, request.get_ref()).await;
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let opt = cli::Options::parse();
    let addr = ([0, 0, 0, 0], opt.port).into();
    let kubeapps_oci_catalog = KubeappsOCICatalog::default();

    let server = Server::builder()
        .add_service(OciCatalogServer::new(kubeapps_oci_catalog))
        .serve(addr);
    log::info!("listening for gRPC requests at {}", addr);
    server.await.expect("unexpected error while serving");
    Ok(())
}