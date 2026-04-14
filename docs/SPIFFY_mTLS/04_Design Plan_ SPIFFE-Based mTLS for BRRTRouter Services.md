# Design Plan: SPIFFE-Based mTLS for BRRTRouter Services

# 1. SPIFFE/SPIRE/SPIFFY Primer

SPIFFE IDs and SVIDs: SPIFFE (Secure Production Identity Framework for Everyone) defines a standard identity string called a SPIFFE ID with format spiffe://<trust-domain>/<path> 1 2 . The trust domain is the root of trust (e.g. an organization or environment), and the path uniquely identifies a workload within that domain 2 . For example, spiffe://prod.acme.com/backend-payment might identify a payment service in the "prod.acme.com" trust domain. A SPIFFE Verifiable Identity Document (SVID) is a cryptographically verifiable token that proves a workload's SPIFFE ID 3 . There are two SVID formats: X.509 SVIDs (X.509 certificates with the SPIFFE ID in the URI Subject Alternative Name) and JWT SVIDs (JWT tokens with SPIFFE claims) 4 . Each SVID carries exactly one SPIFFE ID (the identity of the presenting workload) 3 .

X.509 SVID vs JWT SVID: An X.509 SVID is a short-lived certificate (typically hours) containing the SPIFFE ID as a URI SAN, signed by a trusted CA in the trust domain $^{5}$ . It comes with a private key and a certificate chain up to a trust domain root (the trust bundle) $^{6}$ . X.509 SVIDs are used for mutual TLS authentication: they allow establishing TLS channels with strong identity on both sides. JWT SVIDs are JSON web tokens carrying the SPIFFE ID in the sub claim, signed by the trust domain's key $^{7}$ . JWT SVIDs are handy for attesting identity at the application layer (e.g. in HTTP headers), but are susceptible to replay attacks if intercepted $^{8}$ . The SPIFFE standard recommends using X.509 SVIDs for authenticating channels whenever possible, falling back to JWT SVIDs only when mTLS is not feasible (e.g. if an L7 proxy terminates TLS) $^{8}$ . In practice, JWT SVIDs have a shorter trust radius (they're bearer tokens) whereas X.509 SVIDs enable true end-to-end zero-trust connections.

Workload Attestation vs Cert Distribution: A core SPIFFE principle is that workloads obtain identities via workload attestation rather than static credential distribution. In a SPIFFE/SPIRE environment, a SPIRE Agent running on each node automatically authenticates workloads (using methods like platform attestation or Kubernetes pod identity) and requests SVIDs for them. The workload doesn't need any pre-shared key or manual cert provisioning - it just connects to the local Workload API to get its certificate 9. This contrasts with traditional cert distribution, where certificates or keys are manually injected via configuration or secrets. Why this matters: Attestation-based issuance ensures that only authorized workloads (e.g. with the right Kubernetes service account, or running on an authenticated node) get a certificate. It eliminates human error in secret distribution and greatly reduces the chance of credentials being stolen or misused. Moreover, SPIFFE mandates short-lived certificates that are automatically rotated to limit exposure from key compromise 10. In summary, SPIFFE/SPIRE provides a robust zero-trust foundation: each service proves its identity via an issued SVID, and every connection is mutually authenticated and encrypted with no static secrets 6 10.

SPIRE: SPIRE is the open-source reference implementation of SPIFFE. A SPIRE deployment includes a central SPIRE Server (certificate authority for a trust domain) and SPIRE Agents on each node. Agents attest

workloads and fetch X.509 SVIDs and JWT SVIDs on their behalf. The SPIRE Server maintains the trust domain's root keys and signing keys, and can federate trust across domains if needed. Workloads interact with SPIRE via a local UNIX domain socket (Workload API). This arrangement provides strong guarantees that any presented SPIFFE ID was issued to a legitimate workload (assuming the attestation mechanism is secure) 11 12 .

SPIFFE in mTLS: In an mTLS handshake, the server and client exchange X.509 cert chains. With SPIFFE, those certs include SPIFFE IDs as URIs, and each side validates that the peer's certificate chains to a trusted root and contains an expected SPIFFE ID. The trust root is usually the SPIRE Server's CA (or an intermediate). A trust bundle is the set of root CA certificates for one or more trust domains, which workloads use to verify each other's SVIDs 6 13 . By configuring each service to trust only the SPIFFE trust bundle (and no public CAs), we ensure that only workloads with an SVID from our trust domain can connect. The SPIFFE ID in the cert can further be used for fine-grained authorization (e.g. allow only specific services to call certain endpoints).

Summarizing Zero-Trust Benefits: Adopting SPIFFE IDs with mTLS means each service-to-service connection is strongly authenticated at the connection level. Unlike network perimeter security, this model assumes the network is untrusted - every call, even within the cluster, requires mutual auth. This is critical in multi-tenant and high-security environments (like finance SaaS) because it prevents impersonation, eliminates trust in network location, and drastically limits the blast radius of compromised credentials. In short, SPIFFE + mTLS provides the identity guarantee ("I know exactly what service I'm talking to, cryptographically") and the encryption ("no one can eavesdrop or tamper in transit") that underpin zero-trust architectures 10 6 .

# 2. Recommended Architecture for Workload Identity and mTLS

We evaluate three architecture options for incorporating SPIFFE-based workload identity and mutual TLS in BRTRouter-based services. Each option uses SPIFFE IDs but differs in how certificates are issued and managed:

# Option 1: SPIRE Native Issuance (SPIRE Server + Agents)

Architecture: Deploy SPIRE Server as an internal CA and identity manager, and SPIRE Agents on every node (or as a daemonset in K8s). SPIRE will attest workloads (e.g. using the Kubernetes service account and other selectors) and issue X.509 SVID certificates to each service instance. Services (including BRTRouter frontend and internal microservices) use these certificates for all inter-service TLS. No manual certificate generation - everything is automated via SPIRE's Workload API.

Trust Domain Design: Configure a SPIFFE trust domain for the organization or environment, e.g. spiffe://rerp.prod for production. SPIRE will issue identities within this domain. A common pattern in Kubernetes is to align SPIFFE IDs with namespace and service account. For example, a service running in namespace "accounting" with serviceAccount "bff-service" might get ID spiffe://rerp.prod/ns/ accounting/sa/bff-service 14 15 . Istio and Linkerd follow this convention (trust domain + Kubernetes namespace/SA) 14 , and we can adopt it to ensure uniqueness and traceability of identities. This means each microservice instance's certificate URI SAN encodes exactly which K8s service it is.

Identity Verification & Spoofing Prevention: BRRTRouter (and services built on it) must verify that any client certificate is signed by the SPIRE trust domain's CA and contains an allowed SPIFFE ID. In this option, the trust bundle is just the SPIRE root CA (or intermediate). We'll restrict TLS trust to that bundle – meaning only certificates issued by our SPIRE server are accepted. This prevents external or spoofed certs (an attacker with a public CA cert gets no access because public CAs aren't in our trust bundle). Furthermore, SPIRE won't issue an SVID unless the workload passed attestation (e.g. it's running with the correct K8s service account or on a trusted node), so it's hard for a rogue workload to obtain a valid cert. During TLS handshakes, our server code will extract the client's SPIFFE ID from the certificate's URI SAN and enforce that it matches expected values or patterns (e.g. trust domain must be "erp.prod" and perhaps certain service names only) $2$ $3$ . This check, combined with mutual TLS, ensures both sides authenticate each other. For example, Service A connecting to Service B will verify B's certificate chains to the "erp.prod" root and B's SPIFFE ID = spiffe://erp.prod/ns/...? (correct trust domain and possibly specific allowed service). Likewise B verifies A's cert.

Certificate Rotation & Revocation: SPIRE issues short-lived certificates (by default 1 hour) and rotates them automatically via the agent. Workloads continuously get updated certificates before expiration, typically without needing a restart. The BRRTRouter services would fetch and cache their current SVID from the SPIRE Agent (via Workload API or a mounted volume if using something like spiffe-csi). No hard-coded certs = no stale credentials. If a secret key is compromised or a workload is misbehaving, you can evict its identity: instruct SPIRE to ban or revoke that SPIFFE ID (e.g. by deleting its registration). SPIRE can push a revocation (it doesn't use CRLs, but by not reissuing the cert and using short TTL, the cert naturally expires soon; SPIRE's JWT revocation can also blacklist tokens) 16 17 . One challenge is immediate revocation of an in-flight cert - since X.509 has no instant revocation by default, we mitigate by short lifetimes. Additionally, for disaster recovery, maintain procedures to rotate the trust domain CA itself (SPIRE allows rotating signing keys). That involves distributing the new CA to all clients (which we handle via updating trust bundles on all services, ideally automatically - see "trust-manager" in Option 2).

Pros & Cons: Pros: Strongest security posture - attestation ensures only authorized workloads get identities. Integration with SPIRE means minimal manual overhead; it's a proven CNCF project for production. It supports advanced scenarios like multiple attests (e.g. require both K8s and host attestation for extra security) and federation (if you ever need multi-cluster trust bridging) $^{18}$ . Cons: This adds infrastructure - a SPIRE server (HA recommended) and agents. There's an operational overhead in managing SPIRE (though it's not huge, it's another moving piece). Also, developers must integrate with the Workload API (or we use a CSI driver to simplify that). But since we require high security, this overhead is justified. Another consideration: SPIRE by itself won't manage trust bundles in your app config - the app either queries the Workload API for the bundle or you integrate with something like spiffe-helper or CSI (see Option 2). Overall, Option 1 is best when you want a turnkey zero-trust system and are willing to deploy SPIRE to manage identities continuously.

# Option 2: Cert-Manager Issuer as Internal CA (SPIFFE-ID Certificates via K8s)

Architecture: Use Kubernetes cert-manager to run an internal Certificate Authority and issue per-service certificates embedding SPIFFE IDs. In this model, we don't use SPIRE; instead, cert-manager's Kubernetes native workflow handles cert issuance and renewal. We'd set up a ClusterIssuer representing our CA. This could be: - A simple CA backed by a key pair stored in a K8s Secret (using cert-manager's built-in CA issuer). - Vault as a CA (cert-manager Vault issuer) if we have HashiCorp Vault already for PKI. - Step-CA (Smallstep) via cert-manager's step-issuer plugin for an automated CA with ACME-like flows. Regardless of backend,

cert-manager will be the brain automating cert issuance. We will likely leverage the Jetstack CSI driver for SPIFFE (csi-driver-spiffe) to deliver certificates to pods automatically.

SPIFFE-like Identity Mapping: Without SPIRE, we must map workloads to identities ourselves. The csi-driver-spiffe plugin does exactly this: when a pod starts, it uses the pod's service account and other context to create a CertificateRequest for a URI SAN spiffe://<trust-domain>/ns/<namespace>/sa/ <serviceAccount> 19 . This matches the same SPIFFE ID format as Option 1. The CSI driver runs as a DaemonSet on each node and generates a keypair in-memory (tmpfs) for the pod 20 21 . It then requests a cert from cert-manager, and mounts the signed cert and key into the pod's filesystem (e.g. at /var/run/ secrets/spiffe.io/ by convention) 22 23 . This certificate is an X.509 SVID in everything but name: it's signed by our internal CA, has a short lifetime, and has exactly one URI SAN with the SPIFFE ID (the CSI approver component ensures the CSR is valid - e.g. it requires the URI SAN to match the pod's service account SPIFFE ID and forbids other SAN types) 19 . The trust domain would be configured (e.g. "erp.prod" or "example.com") when deploying the CSI driver 24 . All certificates issued will have spiffe://rerp.prod/... as their identity.

Trust Domain and Bundles: We consider the trust domain the same as Option 1 (e.g. rerp.prod or company.local). The internal CA we configure will act as the root of that trust domain. That CA's certificate (the trust anchor) needs to be distributed to all workloads so they can trust each other. Here we'd use cert-manager's trust-manager or a similar mechanism. Jetstack's trust-manager can continuously combine CA certificates and distribute them cluster-wide as ConfigMaps or Secrets $^{25}$ $^{26}$ . For example, we create a Bundle custom resource to collect our ClusterIssuer's CA cert (available in the issuer's secret) and output it to a ConfigMap that is mounted by all pods as /etc/spiffe/trust-bundle.pem $^{27}$ $^{28}$ . This ConfigMap would contain the PEM of the internal root CA (and any intermediates). Thus, every service has two things mounted: its own keypair (tls.crt, TLS.key) and the trust bundle of allowed roots. BRRTRouter will be configured to use only that trust bundle when validating TLS, not the system CAs.

Verification and Security: On startup, each service (BRRTRouter gateway or internal API) loads its certificate and key from the CSI volume and configures its TLS listener to require client certificates signed by the internal CA. When a peer connects, the rustls/TLS layer will verify the client's certificate against the root CA from the trust bundle, and then our code will verify the SPIFFE URI SAN. Because cert-manager's approver already ensured each cert has a single URI SAN and correct key usage (digitalSignature + keyEncipherment, and both clientAuth and serverAuth EKUs) $^{19}$ , we can trust that format. We'll implement additional authorization logic: e.g. mapping SPIFFE IDs to allowed services or roles. For instance, Service B might only accept client certs whose SPIFFE path equals "/ns/finance/sa/serviceA" if only Service A should call it. This allowlist can be configured in BRRTRouter (a static ACL mapping service names to allowed SPIFFE ID prefixes). Absent explicit allowlists, at least enforce trust domain and maybe namespace restrictions. By default, requiring the trust domain to match (e.g. only spiffe://rerp.prod/\* accepted) prevents anything outside our environment from calling in $^{29}$ $^{30}$ . We also prevent certificate spoofing: an attacker with a cert from another CA (e.g. Let's Encrypt) gets nowhere, because our server doesn't trust those CAs. An attacker who somehow compromises our internal CA could issue bogus identities - mitigating that requires protecting the CA's key (for example, using an offline root or hardware security module if possible, or Vault with proper access controls).

Rotation and Revocation: Cert-manager and CSI driver handle rotation. The default lifetime for CSI-driver-spiffe issued certs is short (the approver enforces e.g. 1 hour by default) $^{19}$ . The CSI driver will request renewal before expiry and mount the new cert/key in place $^{31}$ , $^{32}$ . BRRTRouter should support hot-

reloading the cert (Optionally, we can implement watching the cert file and reloading the TLS config when it changes). If a certificate expires (e.g. if cert-manager were down and renewal failed), the service's TLS handshake will start failing - we need monitoring to catch this (see Runbook). Revocation in this setup (without SPIRE) is trickier: if a specific service's key is compromised, simply waiting for expiry (1 hour) might be acceptable given short lifetime. We could also manually remove its Certificate (or label it to stop renewal) so it doesn't get refreshed. Traditional CRLs/OCSP are not used by cert-manager by default. If a CA compromise occurs (worst case), we'd have to rotate the CA: deploy a new CA, have trust-manager distribute the new root alongside the old (trust both), re-issue all workloads' certificates under the new CA, then remove the old CA from the bundle. This process can be orchestrated to avoid downtime (two sets of roots during transition) 33 34 . Trust-manager makes it easier to roll out trust bundle changes quickly across the cluster 35 36 .

Pros & Cons: Pros: Leverages existing K8s tools (cert-manager) – likely simpler to adopt if the team is familiar with cert-manager. No separate SPIRE server component; the CA can be Vault or even a self-signed root in K8s. The CSI driver approach provides nearly the same security properties as SPIRE (unique per-pod cents, automatic rotation 20 23). Integration with BRRTRouter is straightforward (just load cents from disk). Cons: Slightly less robust attestation – CSI driver by default uses the Kubernetes service account token as proof of identity for the pod to request a cert 37. This is usually fine (it's what SPIRE's k8s attestor also uses), but it means the K8s API and token issuance are part of the trust chain. It's not as extensible as SPIRE's many attestation plugins. Also, if not using Vault or external CA, storing the root key in a K8s secret can be sensitive (mitigate by RBAC and perhaps using KMS integration for secrets). Another con: Revocation is mostly via short lifetimes rather than on-demand. However, given the high-security context, a 1-hour cert lifetime is short enough that revocation is not a big issue (and we can always manually intervene by deleting pods or secrets if needed). Option 2 is a solid compromise when you want SPIFFE benefits but prefer using the K8s ecosystem you already have rather than introducing SPIRE.

# Option 3: Service Mesh (Istio/Linkerd) with Built-in SPIFFE IDs

Architecture: Employ a service mesh which automatically secures all service-to-service communication with mTLS. Meshes like Istio and Linkerd issue certificates to sidecar proxies (Envoy in Istio, or Linkerd's micro-proxy) using SPIFFE IDs. For example, Istio by default issues identities like spiffe://<trust-domain>/ns/<namespace>/sa/<service-account> to each proxy 14 . The proxies terminate TLS to each other, so the applications (the BRRTRouter-based services) see plain HTTP traffic from localhost. The mesh's control plane handles certificate rotation and distribution of trust anchors.

Workload Identity & Trust Domain: We would configure the mesh's trust domain to "rerp.prod" or similar to unify with our SPIFFE model. The mesh control plane (Istio Citadel or Linkerd Identity) acts as the CA. It may use its own root certificate or integrate with cert-manager. In Istio's case, Citadel can be replaced or supplemented by SPIRE as well 11 12 - but let's consider the default: Istio itself issues the certs. All proxies share the trust domain's root CA. The result is each proxy verifies the other via SPIFFE URI SAN (Istio Envoy proxies actually perform SPIFFE URI checks by default, ensuring the presented certificate's URI SAN matches the allowed service account and namespace) 14 .

Mutual TLS Enforcement: With a mesh, you typically enforce mTLS by mesh policy (DestinationRules/ PeerAuthentication in Istio). All traffic between services is transparently TLS-encrypted and authenticated by the proxies. The application code (BRRTRouter) might not even know it's over TLS - it just connects to http://serviceB and the sidecar upgrades it to mTLS. Istio generates policy to only allow traffic from

certain identities if configured (e.g. you can define Istio AuthorizationPolicies that specify allowed client identities for an HTTP route). This covers authorization similarly to what we'd implement in-app for Options 1 or 2, but at the mesh level.

Rotation & Management: The mesh control plane handles rotation (Istio's cents typically have a 90-day life by default, rotated ~every 45 days by default, configurable; Linkerd uses ~24h lifetime by default). The mesh will live-reload certificates in proxies without dropping connections. Trust anchor rotation (e.g. changing the root CA) is supported via mesh upgrades or using intermediate CAs.

Pros & Cons: Pros: Easiest for application developers – no changes needed in BRTRouter code to handle TLS keys or client cert verification; the mesh takes care of it. Also provides lots of features (traffic policy, retries, telemetry, etc.) beyond security. Cons: There is a high cost in complexity and performance. Running Istio, for instance, adds sidecars to every pod (increasing resource usage and ops overhead). It might be overkill if our main goal is just mTLS and we already have a gateway (BRTRouter) and other observability tooling. Also, debugging can become complex (traffic goes through proxies). For a high-performance environment, the added hop in each call path can be significant. Mesh solutions also tie you to their ecosystem; if we want a more lightweight or custom solution, Options 1 or 2 may fit better. Notably, since BRTRouter emphasizes performance (800+ connections via coroutines etc.), adding sidecars could negate some of that benefit. If the team is small, adopting a full mesh might be a heavy burden unless they already planned to.

Trade-off Summary: - Option 1 (SPIRE): Best security, full SPIFFE alignment, independent of K8s specifics; moderate ops overhead. Good for long-term robustness and multi-environment consistency (works on-prem, across VMs, etc., not just K8s). - Option 2 (cert-manager): Leverages familiar K8s tools, likely simplest to implement given current stack. Slightly less extensible but covers our needs. Works on-prem (doesn't need public ACME). Might be the sweet spot for now. - Option 3 (Mesh): Minimizes app changes but adds significant infrastructure (and may be redundant with BRRTRouter's gateway features). If a service mesh was already desired for other reasons, it's a viable way to get mTLS with minimal custom code. But if not, introducing it just for mTLS is likely overkill in this scenario.

Recommendation: Option 2 is recommended for immediate adoption: it is Kubernetes-friendly and doesn't require deploying SPIRE components from scratch. It gives us SPIFFE-compliant identities and mTLS, and we can implement the necessary checks in BRTRouter easily. Over time, if needs grow (e.g. non-K8s workloads, stronger attestation), we can transition to Option 1 (SPIRE) possibly in hybrid (SPIRE can integrate with cert-manager via CSI driver as well) 11 38. Option 3 is not recommended unless we see a clear need for a full service mesh beyond security. It's noted here for completeness and in case the environment already uses Istio/Linkerd.

# 3. Certificate Management with cert-manager: Issuance, Rotation, Trust Bundles

Using cert-manager (as in Option 2) requires a solid strategy for issuing certificates, rotating them, and distributing trust anchors:

- Issuer/ClusterIssuer Choices: We need an internal Certificate Authority. One simple approach is to generate a self-signed root certificate (2048+ bit RSA or ECC P-256) and store it as a secret; then

define a cert-manager ClusterIssuer of type CA pointing to that secret. For higher security, we could use Vault as the CA: Vault's PKI engine can be a root or intermediate CA, and cert-manager's Vault issuer plugin will request certificates from it (this offloads key management to Vault HSM and allows CRL checking if configured). Another option is Smallstep's step-ca which provides an ACME-like API and can do workload attestation; cert-manager has a Step-issuer that could tie into that. However, since we plan to use the CSI driver, the easiest integration is using the CA Issuer or Vault. We will have a single ClusterIssuer that all namespaces can use (hence cluster-scoped). For example, ClusterIssuer: spiffe-ca with spec: ca: { secretName: spiffe-ca-root } (the secret contains the root CA cert and key). In on-prem environments, this does not rely on any external service. It's fully internal, which aligns with our requirement to not depend on public ACME.

- Certificate issuance (pod level): With the spi-diver-spiffe, pods implicitly get certs. Under the hood, when a pod with the CSI volume starts, the driver creates a CertificateRequest CSR object in K8s with an annotation marking it as from SPIFFE CSI $^{37}$ $^{19}$ . The CSR includes the URI SAN for the pod's SPIFFE ID. The cert-manager approver-spiffe (installed alongside the driver) sees this and approves it after validating the content (ensuring the URI SAN corresponds to the pod's actual service account and the requested TTL is acceptable) $^{19}$ . Then the cert-manager controller signs it using the configured ClusterIssuer (our CA) and produces a Certificate. The CSI driver picks up the signed cert and writes it into the pod volume. This all happens rapidly on pod startup. No human intervention. To configure this, we annotate pods (or better, define a CSI inline volume in the deployment YAML). For example:

```yaml
volumes:  
- name: spiffe-credits  
csi:  
    driver: spiffe.csi.cert-manager.io  
    readOnly: true  
volumeAttributes:  
    csi.cert-manager.io/issuer-name: "spiffe-ca"  
    csi.cert-manager.io/issuer-kind: "ClusterIssuer"  
    csi.cert-manager.io/issuer-group: "cert-manager.io"  
    csi.cert-manager.io/trust-domain: "rerp.prod"  
volumeMounts:  
- name: spiffe-credits  
    mountPath: "/var/run/secrets/spiffe.io"  
    readOnly: true 
```

The above ensures each pod gets /var/run/secrets/spiffe.io/tls.crt (cert), TLS.key (private key), and possibly ca.crt (chain). The CSI driver documentation indicates it currently focuses on mounting the cert and key 23 31. We will use trust-manager to mount the CA separately (see below).

- Distributing Trust Anchors: All services need to trust the CA. We have a few options:   
- Static ConfigMap: Manually create a ConfigMap with the CA cert and mount it to every pod at, say, /var/run/secrets/spiffe.io/ca.crt. But keeping this up to date (especially if the CA rotates) is manual and error-prone.

- Cert-Manager's CA injection: cert-manager has a component called cainjector, but that's more for injecting CA into webhook config, not into arbitrary pods.   
- trust-manager: This is an official solution to manage trust bundles dynamically. We deploy trust-manager and create a Bundle CR that sources from our CA secret. For example 39 40 :

```yaml
apiVersion: trust_cert-manager.io/v1alpha1  
kind: Bundle  
metadata:  
    name: spiffe-trust-bundle  
spec:  
    sources:  
        - secret:  
            namespace: cert-manager  
            name: spiffe-ca-root # contains the root CA cert  
            key: TLS.crt  
target:  
    configMap:  
        name: spiffe-trust-bundle  
        key: bundle.pem  
# target namespace can be "spiffe-trust" or we make it cluster-wide accessible 
```

trust-manager will create (and update) the ConfigMap spiffe-trust-bundle containing the concatenated certs from sources (in our case, just one source: the root CA) $^{27}$ $^{28}$ . We then mount this ConfigMap into all pods (or at least all pods that initiate TLS connections). Because our use-case is mutual, ideally both clients and servers have the bundle (servers need it to verify client certs, clients need it to verify server certs). We might mount it at a standard path, e.g. /etc/spiffe/bundle.pem. In BRRTRouter config, we'll set that path as the trust anchors for TLS.

If the CA changes (e.g. we add a new root or intermediate), trust-manager updates the bundle and the ConfigMap volume will update in pods (mounted configmaps update periodically or on fs notify). We should ensure our TLS code can reload trust anchors if needed. In practice, CA rotation is rare; we might simply restart pods if we ever change the CA. But trust-manager at least ensures new pods get the right bundle. The key is that trust-manager eliminates forgetting to update trust bundles - it's automated from the source of truth (the CA secret) 35 36

- Consumption of Certs in Services: With the above in place, each service has:

- t1s.crt / t1s.key (its identity) - unique per pod.   
- bundle.pem (the root CA(s)). We will implement BRRTRouter such that if a service is configured to use SPIFFE mTLS, on startup it reads these files and builds a TLS configuration (see Implementation Plan). For Rust services, we'll use rustls libraries to load the keypair and the root CA into a ServerConfig/ClientConfig. We must also handle hot reload: both certs and bundle can change. A simple approach is to have the service periodically (or upon signal) check the file

Timestamps and reload if they changed. Another approach is to integrate with Kubernetes signals (e.g. SIGHUP on config change) or use a file watch. Given the importance of continuous availability, we'll implement hot reload for certs to avoid restarting pods for routine rotations (detailed in Implementation Plan). Rustls supports swapping the cert resolver or using a custom certificate verifier, which we can leverage.

- Key Usages and SANs: By default, the CSI driver ensures the CSR requests both Client Auth and Server Auth in the Extended Key Usage, meaning the cert can be used by a server (to prove its identity to clients) and by a client (to authenticate to servers) $^{19}$ . This is important for mutual TLS, where each side is both a TLS server and TLS client in different contexts. We should double-check this in the approver policy (the excerpt shows it enforces presence of those EKUs $^{19}$ ). Also, the approver ensures that the cert only contains a URI SAN (no DNS, no email, etc) $^{41}$ . This matches SPIFFE best practices – the identity is in the URI SAN exclusively. We will configure our Rust TLS stack to require a URI SAN and ignore DNS names for authentication. This avoids pitfalls where rustls/ hyper might try to do DNS name matching against the certificate's CN or DNS SAN. Instead, we'll perform SPIFFE URI validation (e.g. ensure the URI SAN trust domain matches ours and optionally the path is expected).   
- Rotation Cadence & Failure Modes: We plan for short-lived certs (e.g. 1h). This greatly limits damage from credential leakage, but it means the system must handle renewals robustly. If cert-manager or CSI driver is down for an extended period, certs could expire and cause outages. To mitigate this:   
- We can extend lifetime moderately (e.g. 24h) in less critical envs, but for prod high-security, 1h is fine as long as the infrastructure is stable.   
- Ensure cert-manager and CSI driver are highly available (multiple replicas, etc).   
- Monitor certificate age. We can instrument a metric: each service can expose the expiry timestamp of its current cert (and of its trust bundle's CA). Our monitoring can alert if a cert is near expiration without a refresh. This would catch, say, cert-manager being down or the CSI driver failing to renew.   
- If a certificate does expire, the TLS handshake will fail – the server (rustls) will reject an expired client cert, and likely the client would also reject an expired server cert. In such a scenario, the services effectively can't talk. The runbook should include steps to quickly refresh certificates (e.g. restart pods to force re-request, or manual Certificate creation if needed).

In summary, the cert-manager + CSI + trust-manager stack gives us automated issuance and rotation of mTLS certificates and automated distribution of trust bundles, which is exactly what we need for hands-off, scalable management of internal identities.

# 4. Let's Encrypt / ACME Usage Boundaries

Let's Encrypt (LE) is a fantastic public CA for securing public web endpoints, but it is not designed for internal service-to-service mTLS in a microservice architecture. Here's explicit guidance on where LE/ ACME fits in our system:

- Use Let's Encrypt for external-facing TLS: If BRRTRouter is acting as a public gateway (front-door) exposing HTTPS to end-users or web clients, it makes sense to use Let's Encrypt certificates on those public endpoints. LE provides free automated certs trusted by all major browsers – perfect for public

HTTPS. For instance, the BRRTRouter front-end could use LE to secure api.myproduct.com for external traffic. Cert-manager can automate that via an ACME Issuer (with HTTP-01 or DNS-01 challenges) if desired. This confines LE usage to the ingress layer (where the identity being asserted is a DNS name like api.myproduct.com).

- Do NOT use Let's Encrypt for internal mTLS: There are several reasons:

- Domain Validation and Reachability: LE issues certificates based on proving control of a public DNS name or IP. Our internal services often use cluster-local hostnames (e.g. Kubernetes service DNS like payment.prod.svc.cluster.local) or no stable DNS at all. We could theoretically use a publicly-resolvable name for each internal service, but that exposes internal topology and may not be feasible (especially on-prem with no public DNS). ACME HTTP challenges would require exposing a port for each service externally (not acceptable), and DNS challenges require managing DNS records for each service instance – highly impractical. LE will not issue certs for names that aren't globally unique domain names or global IPs. For example, it won't issue for *.svc.cluster.local or an IP in 10.0.0.0/8.   
- Rate Limits: Let's Encrypt heavily rate-limits certificate issuance. Currently, it allows 50 certificates per week per registered domain (and 5 duplicates per week) $^{33}$ $^{34}$ . In a microservice environment with potentially dozens of services and frequent deploys (hence new certs), these limits could be hit quickly. Also, each service certificate might need multiple SANs or renewal every 60-90 days – juggling that within 50/week is risky. While LE provides a staging environment for testing, in production these limits are firm $^{33}$ . Hitting them could block new deployments from getting certs. Conversely, internal CAs under our control have no such limits (or we set our own policies).   
- Operational Fragility: Relying on LE means relying on external connectivity (the ACME server, external DNS) and a third-party's uptime. On-prem networks might have restricted internet access, making the ACME challenge or renewal calls unreliable. If an on-prem environment is offline or firewalled, ACME will simply fail. In contrast, an internal CA continues to work in isolation.   
- Trust and Identity Semantics: LE certificates assert ownership of a domain name, not a workload identity. They don't convey a SPIFFE ID or any notion of service identity beyond the DNS name. Using DNS names as service identity is brittle – it doesn't inherently tie to deployment attributes (a compromised service could potentially request a cert for someone else's name if DNS is misconfigured). With SPIFFE, identities are first-class and tied to the workload's identity, not just network location. In a zero-trust setup, we want identities that are meaningful internally (like spiffe://prod/ns/foo/sa/service), which LE cannot provide.   
- Lifetime and Rotation: Let's Encrypt certificates last 90 days. That's relatively long for internal credentials (we prefer hours/days). While one could script monthly rotations, it's slower than the pace of ephemeral containers in Kubernetes. Internally, short-lived certificates (e.g. 1 day or less) are preferable for security; LE cannot practically issue millions of certificates at that granularity for us (nor would we want to hammer their service).

Edge Cases (LE for internal): About the only scenario one might consider LE for an "internal" service is if you had some static internal server that external clients still need to trust (and you didn't want to distribute a custom CA to those clients). For example, if you had an on-prem server that external mobile apps connect to, you might use LE to avoid custom CA on the app. But that's not really internal service-to-service – that's an external client scenario. In the context of microservices calling each other, there's no benefit to a globally trusted cert; it's actually a liability because it means if compromised, that cert could be used to impersonate

your service to any external client that trusts public CAs. We want limited-scope trust (only our system trusts itself).

Final Recommendation on ACME: Limit Let's Encrypt to the public ingress layer (e.g. user-facing HTTPS). Internally, use a dedicated CA (SPIRE or cert-manager CA). This ensures we're not dependent on public infrastructure for our core system security. If we do use ACME, it could be an internal ACME server (like running Step CA in ACME mode or using HashiCorp Vault with ACME) for automation, but that's essentially still an internal CA scenario. For completeness: If one did try to use Let's Encrypt for internal services, you'd have to assign each service a public DNS name and open ports or manage DNS challenges - a huge, unnecessary headache with many failure modes (not to mention broadcasting your internal hostnames to the public). In our high-security, possibly air-gapped contexts, that's a non-starter.

In summary, Let's Encrypt belongs at the edge, providing public TLS certificates for endpoints that external users access (and where using a widely-trusted CA is necessary). It does not belong inside the cluster for mTLS between services. Instead, we rely on our own trust domain and CA which we fully control.

# 5. BRRTRouter Repo Gap Analysis (Current SPIFFE Implementation vs Requirements)

The BRRTRouter repository already includes some SPIFFE-related code, but it appears focused on JWT SVID validation rather than X.509 mTLS. We performed a thorough code inspection and identified gaps in the current security design:

- Lack of X.509 Support (P0): BRRTRouter's SPIFFE support is currently limited to validating SPIFFE JWT tokens (HTTP Bearer tokens) 7 42 . The code in src/security/spiffe deals with JWT parsing, JWKS signature verification, trust domain and audience checking for tokens 43 44 . There is no code to handle X.509 SVIDs or mTLS client certificates. For instance, we searched the repo for any TLS certificate handling and found none - no usage of rustls oropenssl for incoming connections, and no parsing of certificate SAN URIs. The HTTP server code (may_minihttp) is used without TLS termination in the app layer. Gap: Without X.509 support, the system cannot enforce mutual TLS at the connection level - it's relying on bearer tokens which could potentially be sent over unencrypted channels or replayed. This is a critical gap for a "zero-trust" goal. Priority P0 to implement native mTLS.   
- No Mutual TLS Enforcement on Transport (P0): Because the built-in HTTP server does not handle TLS, currently any TLS must be terminated elsewhere (e.g. maybe at a load balancer). That means intra-cluster calls could be happening over plain HTTP. The OpenAPI security schemes supported include HTTP Bearer, API keys, OAuth2, etc., but not the mutualTLS scheme. If a service wants to require client certificates, there is no option in the current SecurityProvider trait. This is a design gap: OpenAPI 3.0/3.1 allow a security scheme type "mutualTLS", but BRRTRouter doesn't seem to implement it (no reference to SecurityScheme::MutualTls in the code). Priority P0 to add a mutual TLS security provider that ties into TLS layer.   
- SPIFFE ID Validation Gaps (P1): For JWT SVIDs, the validation implemented is quite thorough (proper regex for SPIFFE ID, trust domain whitelist, audience check, expiration, signature, etc.) 29

30 . However, if we extend to X.509, we must replicate similar checks on the certificate's SPIFFE URI. Potential gaps:

- The code does not currently parse URI SANs from certificates at all. We need to implement that (using something like rcgen or x509-parser) crate to extract SANs).   
- There is no concept of an allowlist of identities beyond trust domains and audiences. For JWT, trust_domains([...]) is enforced 29, but for X.509 we'd need to ensure the certificate's URI falls under an allowed trust domain (likely same config). That's straightforward. More fine-grained: maybe certain endpoints should only be accessible by certain SPIFFE IDs. That would be a new feature - e.g. an authorization layer that uses SPIFFE ID as principal. Currently, SecurityProvider trait just returns true/false for auth; it doesn't convey "who" the caller is. We might need to extend it to carry the SPIFFE ID to the request context if we want to do RBAC.   
- Hostname/URI verification: In TLS libraries, typically one verifies the server certificate's hostname. Here we'll verify the server's SPIFFE URI instead (the client will check it matches an expected ID or at least the trust domain). Similarly, the server verifying client cert will check SPIFFE URI. We need to ensure rustls is configured to not attempt DNS name verification (which would fail since our certs likely have no DNS SAN), and use our custom SPIFFE validation. This is an implementation detail but crucial.   
- Trust Bundle Handling (P1): Right now, JWT validation uses JWKS for keys 45 46. For X.509, we'll deal with CA certificates. There is no code to load a set of trusted CA certs from config. We need to introduce configuration for specifying the trust anchors (likely file path(s) to PEM bundle, or maybe environment variable with PEM). The absence of this is a gap - if someone tried to do TLS termination in BRRTRouter today, they'd have to fork the server loop to use rustls manually. Gap: No mechanism to supply or reload CA certificates. Solution: Provide config (tls信任 bundle_path) and load it into rustls RootCertStore. Also consider how to update it (e.g. watch for changes).   
- Rotation/Reload Gaps (P1): The existing code doesn't consider credential rotation at runtime. JWT keys (JWKS) do have a refresh mechanism in place (they fetch JWKS with caching) $47 \quad 48$ , which is good. But for mTLS certificates, we'll likely mount them as files. There is currently no observer that would reload a TLS certificate if the file changes. Given the importance of automating rotation, we should implement hot-reload. Without it, a certificate rotation (e.g. via CSI driver) would require restarting the service to pick up the new cert - that's an operational gap. We can address this by either leveraging the existing hot reload infrastructure (there is mention of hot reload for OpenAPI specs in the code) or adding a lightweight file watch on the cert file. Priority P1 to implement cert reload, as it affects availability during rotation.   
- Metrics and Observability (P2): The security code currently logs failures (with $\boxed{\text{warn!}}$ for bad tokens, etc.) $49$ , but there's no integration with metrics for auth failures. For a production system, especially in finance, we want to monitor TLS handshakes and credential status. We should add counters for:   
- Number of mTLS handshake failures (by cause: untrusted issuer, expired cert, invalid SPIFFE URI, etc.).

- Gauge for days/hours until cert expiry of the service's own cert (to alert if it's not rotating). Possibly also an event or metric when a client with an unauthorized SPIFFE ID attempts access (indicative of either misconfiguration or malicious attempt). These aren't present currently.   
- Default Security Posture (P0): We must examine if the current defaults are fail-secure. In the JWT implementation:   
- If JWKS URL is not configured, validate() fails and warns that signature verification is required 17 - so that's good (no accepting unsigned tokens) 51.   
- If trustdomains or audience are not configured, it also fails secure 29 52 (they require explicit config). Those are positive. However, we need to ensure the same strictness for TLS:   
- The system should not start in mTLS mode without trust anchors configured (it should error out if we enable client cert auth but provide no CA, rather than inadvertently trust all certs). By default, rustls won't trust anything if given an empty RootCertStore, which actually is safe (it would reject all clients). That's fine, but better to explicitly require configuration.   
- The cipher suites and protocols: rustls defaults are already modern (TLS1.2/1.3 only, strong ciphers). We should double-check any TLS configuration we expose – e.g. no TLS versions below 1.2, ensure PFS ciphers, etc.   
- One potential weakness: If someone doesn't enable mTLS in config, will the port accept plaintext? Yes, by default currently everything is plaintext. That's okay as long as we document that for internal comms you must enable TLS. Maybe we could consider having an option to require TLS for certain routes (like an OpenAPI mutualTLS scheme enforcement). But a simpler approach is: run two modes - either TLS is on for the entire service port or it's not. In a zero-trust environment, we'd run all internal services with TLS on.   
- Authorization logic (P1): Beyond authentication, zero-trust often implies each service should only accept requests from specific callers (the "least privilege" principle for service interactions). Currently, BRTRRouter's SecurityProvider returns a boolean validate(request). For JWT, if valid, presumably the request is allowed (it doesn't check the content of sub beyond being a SPIFFE ID and within trust domain). That means any SPIFFE ID in the trusted domain with a valid token is accepted equally. There's no attribute-based access control to differentiate service A vs service B tokens. In our mTLS scenario, similarly, any valid cert from the trust domain would be accepted by default. This is a gap if certain services shouldn't talk to others. We likely need to introduce an authorization layer (maybe not in initial P0 implementation, but soon after):   
- For example, a config could specify something like: allowed clients: ["spiffe://rerp.prod/ns/foo/sa/frontend"] on a given service or route. Then the middleware would check peer SPIFFE ID against that.   
- Since this is not present at all now, we mark it P1 (should fix). It's not absolutely required to get mTLS working (mTLS ensures only legitimate identities connect, but it doesn't stop an otherwise legitimate service from calling where it shouldn't). Given multi-tenant concerns, adding this is highly advisable.   
- Code Robustness (P2): The SPIFFE JWT code has thorough checks (even preventing algorithm confusion by matching JWT alg to JWKS alg) $^{53}$ . We should emulate that thoroughness in our certificate handling:

- Ensure to reject certificates that don't have the SPIFFE URI SAN or have more than one SAN (per spec, one SPIFFE ID per cert) – the CSI approver ensures this, but if someone somehow bypassed and added a DNS SAN, we should either ignore it or reject to be safe.   
- Ensure the key usage includes what we require. If some cert came through without clientAuth EKU and it tries to authenticate as a client, rustls by default doesn't check EKUs for client certs (it only checks server cert has serverAuth and client cert has clientAuth by default). We should double-check that logic. Possibly implement a custom verifier to enforce both sides have proper EKUs, to avoid mis-issued cert usage.   
- Revocation: neither rustls nor cert-manager deals with CRLs. SPIRE has an API for JWT revocation which BRRTRouter code includes (an in-memory revocation list) $^{16}$ . For X.509, we might not implement CRL checking initially (we rely on short lifetimes). But if needed, we could integrate a CRL fetch or OCSP check for Vault-issued certificates, etc. This is likely P2 (nice-to-have if we integrate with a PKI that supports revocation lists).   
- Logging: ensure any authentication failure is logged with enough detail (but not sensitive info). Current code logs, e.g., invalid trust domain or audience as warnings $54\quad 50$ , which is good. We should continue that for TLS: e.g., "TLS handshake failed: untrusted issuer" or "client cert SPIFFE ID not allowed:<id>".

Priority Summary: - P0 (Must Fix): Implement mTLS (X.509 SVID) support: TLS listener with client cert auth, parsing/validating SPIFFE URI. Without this, our system cannot be considered zero-trust internally. Also ensure the server uses only trusted CAs (no default system CAs for internal endpoints). Essentially, build the foundation for Option 2 or 1 as chosen. - P0: Integrate TLS at the core HTTP server or provide an alternative server that supports TLS. Possibly replace HttpServerWithHeaders with a wrapper that does rustls accept. This is fundamental. - P1 (Should Fix): Hot reload of certs/keys and CA bundle - to enable rotation without downtime. - P1: Authorization by SPIFFE ID rules - at least at service granularity. This might be configurable allowlists. It significantly hardens security by preventing lateral movement even among valid identities. - P1: Configuration surfaces for TLS: e.g., in config file or env: paths to cert, key, ca bundle; flags to require client cert; allowed SPIFFE trust domains (though could reuse the existing trust_domains config used for JWT). - P1: Update documentation and OpenAPI generation to support mutualTLS security scheme (so that OpenAPI spec can declare an endpoint requires mTLS). This integration will signal to BRRTRouter to use the mTLS SecurityProvider. - P2 (Nice-to-have): Detailed metrics and event logging for mTLS (for monitoring and incident response). - P2: Possibly integration with SPIRE if Option 1 is ever used - e.g., allow a mode where instead of reading files, the service contacts SPIRE Workload API directly. But that can be an extension later. - P2: Robust testing around certificate parsing (e.g., ensure we handle unusual but valid URIs, or long chain lengths, etc).

In conclusion, BRRTRouter's current "SPIFFE support" is only half of what's needed (JWT auth). The critical gaps to fill revolve around actual transport security (TLS) and expanding the trust model from just "token is valid" to "connection is mutually authenticated and authorized." The next section outlines how to implement these fixes.

# 6. Concrete Implementation Plan for SPIFFE mTLS in BRRTRouter

This plan details the steps and design changes required to implement and harden SPIFFE-based mutual TLS in BRTRouter and its associated services. The implementation touches configuration, code (Rust), and deployment aspects:

# 6.1 Configuration and Interface Changes

We will introduce new configuration options to control mTLS. Likely, BRRTRouter uses a config.yaml or similar (as seen in the generated main.rs for RERP, it loads AppConfig from YAML 55 56). We will extend the config schema:

```textproto
security: # ... existing fields (api_keys, oauth2, etc.) mtls: enabled: true # If true, enable TLS on the HTTP server and require client cert cert_file: "/var/run/secrets/spiffe.io/tls.crt" key_file: "/var/run/secrets/spiffe.io/tls.key" caBundle_file: "/etc/spiffe/bundle.pem" # optional: allowed_spiffe_ids: [] # a list of specific SPIFFE IDs or prefixes allowed (if empty, all in trust domain are allowed) trust_domain: "rerp.prod" # expected SPIFFE trust domain for clients (e.g., trust domain whitelist) # (trust_domain could default to the URI in our own cert if not set) 
```

Additionally, we add support for OpenAPI mutual TLS security scheme. Possibly the code generator and router need to handle a SecurityScheme::MutualTLS. We can map that to requiring a client cert. In practice, since we will globally require client cert if mTLS.enabled, we might not need per-route config. But OpenAPI allows marking specific endpoints as mTLS-required. We can interpret that as: if any route requires mTLS, we should run the server in mTLS mode (since we can't have some endpoints with TLS and some without on the same port easily). Alternatively, we can run separate ports (one with mTLS, one without) but that complicates things. Simpler: if any mutualTLS scheme is present in the spec, require mTLS on the whole server. This should be documented.

Code Adjustments for Config: The AppConfig struct in main.rs would get a new mtls: Option<MtlsConfig> field 57 58. We'll propagate that through BRRTRouter library where needed. The HttpServer.start() call site will be modified: instead of directly calling .start(&addr), we will wrap it with TLS if mtls is enabled.

# 6.2 TLS Setup: Server-side

We will use rustls (via the rustls crate and possibly tokio-rustls if using async, but may is not async runtime; we might use rustls in blocking mode). Plan: - Add rustls and webpki crates to BRRTRouter dependencies (if not already). Actually, request was using rustls for JWKS fetch 59, so rustls is indirectly present. But we might want to use a lower-level approach. - In the HttpServer::start() implementation, we can no longer just call may_minihttp directly on a socket address. We need to accept TCP connections, do TLS handshake, then pass the decrypted stream to may_minihttp's HTTP handling. Options: 1. Modify may_minihttp to support TLS sockets (likely complex). 2. Perform TLS handshake in a separate thread for each connection then handoff. 3. Use an alternative server for TLS only. Perhaps use tokio just for the TLS accept, then hand off to may for processing.

Given may is a coroutine library (not compatible with tokio), one approach: - Use std::net::TcpListener to accept connections. - For each accept, do a rustls handshake (which can be done blocking, rustls is not inherently async). - If handshake succeeds, wrap the rustls::Stream (which implements Read + Write) in an object that mimics TcpStream for may_minihttp, or adapt may_minihttp to read from that stream. - may_minihttp's HttpServerWithHeaders likely expects a generic HttpService trait impl. It might allow us to supply a custom listener or stream. If not, we might need to run our own loop.

Alternatively, we could bypass may_minihttp and use hyper with rustls for TLS, but that would throw away the coroutine design and likely impact performance. Since performance is key, let's try to keep may's coroutine server: We can spawn a coroutine that: - Listens on TCP. - For each new connection, spawn another coroutine to handle it: - Do rustls Acceptor.accept() using our ServerConfig. - If client cert is required, rustls will automatically enforce presenting one (we set client_certVerifier accordingly). - Once the TLS session is established, we have a rustls::Stream<ServerConnection, TcpStream>. - We then feed that into the HTTP handling. Possibly, we can implement the may_minihttp::HttpService trait for an object that wraps a rustls stream.

We might need to patch may_minihttp or interpose in the request reading/writing. Perhaps easier: after handshake, read bytes from the rustls stream in a loop and push them into the existing request parsing machinery. Actually, may_minihttp's HttpServerWithHeaders presumably reads from a raw TcpStream. We might be able to mimic that: We could create a struct that implements std::io::Read and std::io::Write by internally calling TLS_STREAM.read and TLS_STREAM.write. Then pass that to the HTTP handling logic. Because may_minihttp's HttpService likely processes one connection per coroutine.

This is fairly involved; as a stop-gap, we could also run a separate process like Envoy to terminate TLS and forward plaintext to our router. But that's essentially a mini service mesh, not desired when we can do it in-process.

Given complexity, an intermediate approach: - Use hyper + hyper-rustls for the server, but that would mean abandoning may and rewriting a lot. That's not desirable for now.

So, we proceed with custom accept loop using rustls: - Create a static rustls::ServerConfig at startup: - Load mtls.cert_file & mtls.key_file. Use rustls::Certificate and rustls::PrivateKey by reading the PEM. (We must support if the cert file has chain and leaf; rustls expects the full chain). - Load mtls.ca Bundle_file into a RootCertStore. Use that to create a AllowAnyAuthenticatedClient verifier, restricted to that root set 33 . - Possibly implement a custom CertificateVerifier to further check the SPIFFE URI. Actually, rustls doesn't parse URI SANs for verification (it only does DNS name verification if you give a ServerName). We can leverage rustls's client_certVerifier to ensure the cert chains properly, then after the handshake, do SPIFFE checks. - Set ServerConfig.client_certVerifier = AllowAnyAuthenticatedClient(rootstore) if we just want to ensure it's signed by our CA. This covers cryptographic authentication. Then after handshake, we

call our own function to inspect the peer cert. - Also set ServerConfig.alpn_protocols = ["h2", "http/1.1"] to allow HTTP2 for gRPC (tonic) and HTTP1 for REST.

- Modify BRRTRouter's startup:

- If mtls.enabled, instead of HttpServer(service).start(addr), call a new function, say TlsServer::start(addr, service, server_config).   
- Implement TlsServer::start to do what we described: accept loop, spawn coroutine per connection, handshake, then call service. The service object likely implements may_minihttp::HttpService (BRRTRouter's Router is an HttpService).   
We may need to adapt the interface because HttpServerWithHeaders normally internally accepts and then calls service. We might have to reimplement that logic externally.   
A simpler hack: use a local pipe. But that's too hacky.

So, code outline in pseudo-rust:

let listener $=$ TcpListener::bind(addr)?;   
let TLS_config $\equiv$ configure_rustls(mtls);   
loop{ let (socket,addr) $=$ listener.accept(？); may::coroutine::spawn(move || { let mut TLS_session $=$ rustls::ServerConnection::new(tls_configClone().unwrap(); let mut TLS_STREAM $=$ rustls::Stream::new(&mutTLS_session，&socket); // Perform handshake (rustls does lazy handshake during first read/write, so we ensure it now) if let Err(e) $=$ tls_sessioncomplete_io(&socket){ log handshake error, close socket; return; } // At this point, handshake is done. If client auth was required and failed,rustls would already have aborted.. // Now get client cert and verify SPIFFE let peer_cert $=$ tls_session.peer_certificates(); if let Some(chain) $=$ peer_certs{ if let Err(err) $=$ verify_peer_spiffe(chain[0], allowed_spiffe_ids_or_trust_domain){ log "Client SPIFFE ID not authorized",close; return; } } else{ log"No client certificate provided (but required)",close; return; } // Now proceed to HTTP handling handle_http_connection(tls_STREAM，service.clone());

```txt
}）; 
```

The handle_http_connection would need to parse HTTP from the tls_STREAM and use the Router. We might call service.handle_request streaming in a loop until EOF. We may leverage existing code by tricking may_minihttp: We could instantiate a may_minihttp::HttpService for our Router and call something like service-process streaming if such API exists. (We'll have to read may_minihttp docs; possibly we need to adapt or copy some code from it.)

This is a complex integration but doable. Another idea: since may_minihttp is not TLS aware, what if we terminate TLS outside and feed plaintext into may_minihttp using a local TCP? That is like running stunnel in-process – probably not worth complexity.

We will proceed with direct approach for the plan (noting it as engineering effort).

Server identity: We should ensure that the server presents the appropriate SPIFFE ID in its own certificate. In our setup, that's already the case (the CSI-issued cert for the server will have its spiffe URI SAN). But if BRTRouter is a gateway, perhaps it might use a public cert for external connections. We might have separate config for external vs internal. For simplicity, we could run two listeners (one on 8443 for internal mTLS, one on 443 for external TLS with a public cert). That would complicate config. Possibly out of scope; we assume internal services use one port with mTLS. If BRTRouter also serves external, maybe run separate instance or have logic to handle both (like require client cert only on internal network). This can be addressed later; focus on internal.

# 6.3 TLS Setup: Client-side

It's not just the servers - when one service calls another (e.g. Service A calling B's REST API or gRPC), the client code must present its cert and validate the server. In a typical microservice, calls might be done with request, hyper, or gRPC (tonic). We must ensure: - Outgoing HTTP client in Rust uses our credentials. We might provide a utility or config to easily create a client. For example, if a service uses BRTRouter's generated client or just request, we can configure request with a client certificate and CA root. We can set environment variables like SSL_cert_FILE pointing to the bundle for libraries that use system TLS (but Rust's native TLS (rustls) doesn't use that env; better to programmatically configure it). - Possibly, we provide within BRTRouter a configured request Client that already has the identity. E.g., BRTRRouter could expose something like brtrouter::mtls::ClientBuilder which reads the same cert, key, and bundle from config and yields a request client or a hyper client with rustls connector. This would simplify service implementation: they call through this and automatically do mTLS. - For gRPC, Tonic allows setting a ClientTlsConfig with identity and CA. The identity is basically the cert+key and CA is the trust anchors. So similar approach: load from file, configure channel. - This isn't strictly BRTRRouter's responsibility, but since it's a platform piece, providing helpers would be good.

We ensure client verification of server: Rustls by default will verify the server certificate against the RootCertStore and require the DNS name matches (if you provide one). In our case, we don't have a DNS name for the SPIFFE cert (only URI). We will likely override hostname verification. Options: - Use rustls ServerName:::try_from("example.com") with some dummy name and include that as DNS SAN in all certs (not ideal). - Write a custom verifier for the client side as well: rustls ClientConfig has

dangerous().set_certicateVerifier(Arc:::new(MyVerifier)). Our verifier will check that the presented server cert chains to our root (we can leverage rustls's default for that part), and then check that the server's SPIFFE URI SAN trust domain == expected. Possibly also check the service's exact ID if we know it. For general calls, we might just check trust domain to avoid coupling clients to server names. But in high-security, a client might be configured to expect a specific SPIFFE ID for a given endpoint (like an authorization of the server identity). This could be configured by service discovery or manually.

Given complexity, an easier interim: allow skipping name verification in rustls (which we can do by providing the root store and then calling config.dangerous().set_certificateVerifier(. . .) with one that only does partial checks). But since our trust domain CA is dedicated, trusting it implicitly is okay as long as any certificate from it is considered valid server. But to be safe, we at least ensure the certificate has a URI SAN with the correct trust domain. This prevents a compromised service from using its cert to impersonate another when a client doesn't know which service it connected (though normally the client knows which host it's calling by context).

- Implementation: Provide a function fn verify_server_cert(cert: Certificate) -> Result<()> that checks the URI SAN's host equals configtrust_domain. Use that in a ClientCertVerifier impl for rustls client.   
- We add to config: perhaps a mapping of URL -> expected SPIFFE ID. For example: service_mtls: [ { host: "https://payment.prod.svc.cluster.local", spiffe_id: "spiffe:// rerp.prod/ns/prod/sa/payment"}]. This could be used by the client to verify it's talking to the right service. This might be overkill; we can assume trust domain enforcement is enough at first.

# 6.4 SPIFFE ID Extraction and Authorization

When a BRRTRouter service receives a request over mTLS, after handshake we will have access to the client's certificate chain (rustls ServerConnection.per_certificates)). We must extract the SPIFFE ID: - Parse the certificate (we can use x509-parser crate or webpki if it supports URI SAN, but webpki might not expose URI SAN easily). Perhaps easier: use rcgen or boring to parse. Actually, rustls provides the certificates in DER. We can use the x509-parser to parse DER and get SubjectAlternativeName extension. It will list general names, including URI. We find the one that starts with "spiffe://". - Extract trust domain and path from that URI.

Then apply authorization rules: - If config specifies allowed SPIFFE IDs or prefixes, check here. For example, BRRTRouter as an API gateway might have a config like allowed clients = ["spiffe:// rerp.prod/ns/frontend/*"] meaning only front-end service accounts can call it. We will support wildcard by simple string prefix match (since SPIFFE IDs are hierarchical). Or we could allow a regex. - If the presented SPIFFE ID is not in the allowed list, we reject the request. How to reject? Since TLS handshake already succeeded, we cannot retroactively fail the handshake. Instead, we can immediately close the connection or, perhaps better, we respond with an HTTP 403 and then close. But the HTTP request is not yet read at that point? Actually, by the time we verify, it's during handshake (if we integrate it there). Alternatively, do it at the beginning of HTTP handling: read the request, and before dispatch, check SecurityContext for client SPIFFE ID. We can integrate this with the existing SecurityProvider trait by adding a new implementation: e.g., struct SpiffeMTLSProvider that implements SecurityProvider such that validate req) returns false if the peer's SPIFFE ID isn't allowed. But SecurityRequest right

now doesn't include peer cert info. We might need to extend it to carry connection metadata. We can extend SecurityRequest to have an optional field for peer_spiffe_id (populated by the server when using mTLS). - Simpler: since the handshake loop is outside of request handling, we can enforce allowed IDs in the handshake coroutine and drop connection if unauthorized (the connection never gets to send any HTTP). This is effective and secure, but it results in a generic TLS handshake failure (the client's connection drops). Alternatively, allow connection but fail each request auth - but that wastes resources. It's better to drop early. We'll opt to drop at handshake if unauthorized ID is detected. In the log, we'll record that event.

- Internally, we should maintain an allowlist data structure (maybe a HashSet of exact IDs, or a list of prefix strings). For efficiency, maybe parse config patterns into a regex or something at startup.

Revocation considerations: If we had an in-memory revocation list (like for JWT jti), for mTLS we'd consider something analogous. But given short cert life, we might skip CRL. If needed, we could integrate with something like OCSP stapling if Vault provides it, but not necessary at first.

# 6.5 Hot Reload of Certificates and Bundles

To avoid downtime on rotations: - Implement a watcher thread (or coroutine). The notify crate can watch file changes. We watch the cert_file and key_file for modifications. When changed, reload the rustls ServerConfig (or rather update the cert chain and key in it). Rustls does allow swapping the certificate at runtime if using a ResolvesServerCert trait, but easiest might be to reconstruct the config and swap an Arc<ServerConfig> that the accept loop holds. We can store the Arc<ServerConfig> in a global protected by RwLock and have the handshake code use the latest each time. - For client side, similarly, if we use a global ClientConfig, refresh it on changes. Or simply create new clients for each request (inefficient). Better to keep a client but update its config - not trivial once constructed. We may just log that a restart is needed if own cert rotates for outgoing calls, but ideally handle it. - Trust bundle reload: trust-manager will update the bundle file when CA changes. We watch ca Bundle_file for modifications and update both server verifier (for new clients connecting) and client config (for outgoing calls). In practice, CA rotation is rare, so manual restart might suffice, but we can implement to be safe.

- Integration with existing hot reload (BRTRouter supports hot reloading the OpenAPI spec on file change). Perhaps we can piggyback on that mechanism (maybe they have a thread already). If not, we'll implement separately.

# 6.6 Logging and Metrics

Add logging statements in the TLS handshake logic: - On handshake success, log debug: "Accepted mTLS connection from [client_addr], SPIFFE ID = xyz". - On failure: - If client didn't provide cert: log warn "mTLS client certificate required but not provided - closing connection" (this might indicate a misconfigured client or attacker). - If untrusted cert: rustls will error; catch it and log at debug or info (it could be a probe). - If SPIFFE ID not allowed: log warn "Rejected client SPIFFE ID not authorized: <id>". - These logs help incident analysis.

Metrics: - We'll increment counters for handshake failures by category. Possibly integrate with Prometheus metrics already present. BRRTRouter has metrics middleware for requests 60 ; we can extend to include TLS metrics. Maybe expose: - brrt_tls_handshake_failures_total{reason="expired"} etc. -

brrt_tls_authz-denials_total for unauthorized IDs. - brrt_tls(connections.active gauge (increment on connect, decrement on disconnect) to monitor usage.

- We can also expose our own cert's expiration as a metric (days until expire). This is very useful to avoid outages.

# 6.7 Code Structure

We might create a new module brrtrouter::mtls or extend security::spiffe module to handle TLS aspects. To avoid confusing it with JWT SpiffeProvider, perhaps a separate security::SpiffeMTLSProvider that implements SecurityProvider but its validate() uses connection context.

We need to propagate the client SPIFFE ID into request context so that handlers or logging might use it. Possibly add a field in RequestContext or a header (though forging header isn't safe, better out-of-band). We could set an HTTP header like X-SPIFFE-ID internally (not from client but by the router) so that the application can know who called it (if needed for auditing). This header approach is sometimes used (e.g., Envoy can propagate peer identity as header). We must ensure it can't be spoofed by client (we'd strip any incoming X-SPIFFE-ID from requests to be safe). This is a design decision to consider for later.

# 6.8 Example Changes

New TLS SecurityProvider: We implement:

```rust
pub struct SpiffeMTLSProvider {
    allowed_ids: Option<String>, // exact IDs or prefixes
    trust_domain: Option<String},
}  
impl SecurityProvider for SpiffeMTLSProvider {
    fn validate(&self, scheme: &SecurityScheme, req: &SecurityRequest) -> bool {
        // ensure scheme is "mutualTLS"
        if !matches!(scheme, SecurityScheme::MutualTls) { return false; }
        if let Some(peer_id) = req.peer_spiffe_id() {
            if let Some(categoryid) = &self.trust_domain {
                if !peer_id.starts_with(&format!("spiffe://{}/","domain")) {
                    warn!("Client SPIFFE ID {} is not in trust domain {}
peer_id, domain);
            }
            return false;
        }
    } 
if let Some(ref allowed) = selfAllowed_ids {
// allowed list can support prefix match like "spiffe://domain/
ns/foo/*" 
let authorized = allowed_iter().any(|pattern| {
if pattern.endsWith(['*']) {
peer_id.starts_with(&pattern.trim_end_MATCHes(['*')) 
```

```txt
} else { peer_id == pattern } }; if !authorized { warn!("SPIFFE ID {} is not in allowed list", peer_id); } return authorized; } // if no allow list, just trust domain check passes => authorized return true; } false // no peer ID (shouldn't happen if mTLS is enforced) } } 
```

We then tie this provider into the router if a route requires mutualTLS (or globally). Possibly simpler: if mtls.enabled, we register this provider globally for any route that has securityScheme: mutualTLS. The router will call validate() on it per request, but the heavy lifting (actual TLS handshake) happened already.

Server main changes: Load config(mtls; if enabled: - Construct rustls config. - Start TLS accept loop instead of normal HttpServer. If not enabled: - Behavior unchanged (no TLS, perhaps use JWT/other providers as usual).

Testing the Implementation: We will write unit tests for: - Parsing a cert and extracting SPIFFE ID. We can embed a sample SPIFFE X.509 cert DER in tests and ensure our parser finds the URI properly. - The allowlist logic (patterns). - That our SpiffeMTLSProvidervalidate returns false for mismatches etc. We also add integration tests: - Spin up a BRTRouter server with mtls enabled and a self-signed test CA, then use a rusts client with a valid cert to connect and send a request, expect 200. Then try with no cert or invalid cert expect connection drop or no response. - Could use tiny_http or similar to simulate a client with cert. - We will need to generate some test certificates for this (could useopenssl in test setup or include static ones).

All tests will be offline (no external calls).

We'll ensure existing tests (JWT etc.) still pass. Possibly they will because we're adding new stuff rather than changing JWT logic.

# 7. Test Strategy for SPIFFE mTLS

Implementing security features requires rigorous testing at multiple levels:

7.1 Unit Tests (Library Level): - Certificate Parsing & SPIFFE Extraction: Create unit tests for a function extract_spiffe-uri_from_cert(cert DER: &[u8]). Use known test vectors: - A valid SPIFFE X.509 cert (we can self-sign one in tests with URI SAN spiffe://example.com/my/service). Ensure the function returns the correct URI 4 . - A cert with no URI SAN (or a DNS SAN) - ensure our function returns

None or error. - A cert with multiple SANs (shouldn't happen in SPIFFE; if it does, we decide to either use the first URI SAN or fail - likely fail for security). Test that we handle that appropriately (probably reject). - SPIFFE ID validation logic: Test the helper that checks trust domain and allowlist: - Allowed trust domain = "example.com", input "spiffe://example.com/foo" $\rightarrow$ OK. - Input with wrong trust domain $\rightarrow$ rejected 54 . - Allowed list patterns: e.g. allow [spiffe://example.com/ns/foo/*], input spiffe://example.com/ns/foo/service1 $\rightarrow$ OK, input spiffe://example.com/ns/bar/svc $\rightarrow$ not OK. - No allowlist but trust domain set: any ID in domain accepted. - No trust domain set but allowlist given: exact matches enforced. - Cert verification logic: If we implement a custom rustls verifier, unit test it by simulating certificate chains (maybe use a dummy self-signed cert as root and a leaf). We can call verifierverify_client_cert(chain, ...) and assert it returns Ok for a valid chain with correct SPIFFE URI, and Err for: - Wrong issuer (chain doesn't lead to our root). - Expired cert (we can create a cert that is expired by altering notBefore/notAfter). - Missing clientAuth EKU if we require it. - URI SAN missing or wrong trust domain. - Revocation list (if any): Not likely since short-lived. If we had a structure for JWT jti revocation, apply similar concept? Possibly not needed for X.509.

7.2 Integration Tests (Local, single process): We can leverage the existing integration test framework (like how they start a server in tests using TcpListener 61, etc.). Plan: - Ephemeral CA and certs: In test, generate a root key+cert (maybe using rcgen oropenssl CLI invoked via std::process). Then generate a client cert signed by it with a SPIFFE URI. - Start a BRTRouter instance (maybe using the pet_STORE example or a minimal router config) with mtls enabled, using those certs. Possibly we can invoke the router on a background thread (or spawn process). - Use a client (maybe request or native rustls) to connect: - Case 1: Valid client cert, correct SPIFFE ID $\rightarrow$ expect request succeeds (we can hit a test endpoint that echoes something). - Case 2: No client cert $\rightarrow$ connection should be rejected (if our server requires a cert, rustls handshake fails). The client (request without cert) should error. We assert the error kind is handshake failure. - Case 3: Wrong trust domain cert: e.g. a client with a cert signed by another CA (not in trust bundle) $\rightarrow$ handshake fails (untrusted cert). - Case 4: Cert valid chain but SPIFFE ID not allowed by allowlist: we can issue two client certs, one with URI allowed and one disallowed (maybe different path). The disallowed one should connect (since TLS sees it as valid) but our code should drop it right after handshake. The client might see either a connection reset or if we choose to respond with HTTP 403 then close. We should assert that the client doesn't get a 200. We might need to parse if error contains "early EOF" to indicate drop. - Case 5: Expired certificate: generate a cert that expired yesterday. Attempt handshake, should fail. The client error should indicate invalid certificate (rustls::Error::WebPki). - Mutual Auth Data Test: If possible, configure server to echo back the SPIFFE ID (e.g., in a response header or body). Then ensure the client receives the correct ID of itself or something? Actually the server could echo the peer's ID in response just to verify server indeed got it. We can implement a dummy handler that returns req.peer_spiffe_id in the body for testing. Check that.

- Kubernetes End-to-End (Simulation): Not easily done in unit tests, but we can simulate parts:   
- Use cert-manager's CFSSL or step CLI to generate example chain. Possibly overkill in code. We might instead trust our unit tests for chain handling.

7.3 Kubernetes E2E Tests: (This might be done in a staging cluster or CI with KinD.) - Deploy cert-manager, csi-driver-spiffe, trust-manager as per our blueprint. - Apply an Issuer and Bundle for test trust domain. - Deploy two test services (one BRRTRouter-based server with a simple echo endpoint, and one simple client job that curls that endpoint). - Configure the server's OpenAPI to require mutualTLS on that endpoint. - The client container should have the CA bundle mounted and use curl with --key/ --cert to connect. - Verify the server logs show the SPIFFE ID and the client got a 200. - Then try to curl from a pod with no cert (just to

confirm it fails). - Also test cross-namespace: if one service is not allowed to call another (simulate by allowlist config), ensure that call fails (maybe a 403). - Test rotation: manually trigger a rotation by deleting the CertificateRequest to force re-issuance, or by rolling the CA (update the ClusterIssuer secret to a new key+cert and see trust-manager update ConfigMap). Ensure existing connections drop (expected since server cert changes, clients need to trust new CA, etc.), and new connections succeed after both sides updated. This is complex to automate, may be done in a manual test scenario.

# 7.4 Failure Mode Tests:

We want to specifically validate the system's behavior under the failure scenarios: - Expired Cert: Simulate that a service's cert expired (perhaps set cert-manager to issue a short-lived cert, then pause the cert-manager so it can't renew, wait for expiry). The expected outcome: the service should start rejecting connections (as its own cert is invalid to clients, or it rejects others if their cert expired). Our monitoring in a test can detect logs "certificate expired" or the metric for near expiry. In a controlled test, we might just manually replace the cert with an expired one and see that our server refuses to start or refuses connections (depending on if it loads at startup or continuously). - Trust Bundle Mismatch: Simulate CA rotation not fully propagated: - Start with CA1 trusted on both sides. Issue certs with CA1. - Rotate server to CA2 (server now trusts CA2 and uses cert signed by CA2). But client still trusts only CA1 (didn't get updated bundle). - Client connects -> should fail (unknown CA). We verify the error is "Untrusted cert". - Then update client's trust to include CA2 (simulating trust-manager update), retry -> should succeed now. This tests our ability to handle intermediate state gracefully (some connections will fail until both sides converge on trust set). - Compromised Certificate Simulation: Hard to simulate detection of compromise. But we can simulate revocation: - If we had an allowlist, "revoke" by removing an ID from allowed list at runtime if we support dynamic config reload (maybe not dynamic in this iteration, likely needs restart or SIGHUP to reload config). Then attempts from that ID are now denied (we can test that). - Actual key compromise detection is outside of app (that's process: if suspected, we'd rotate CA or stop that service).

- Plaintiff Downgrade Attempt: If someone tries to speak HTTP plaintext to the TLS port, rustls handshake will fail (likely the server will send no response or a TLS alert that the client (HTTP) doesn't understand). We can test by netcat or telnet to the TLS port sending a “GET /” without TLS:   
- The expected outcome: either the connection is closed immediately or garbled (which is fine; the main point is it doesn't succeed).   
- This ensures an attacker cannot bypass auth by simply not doing TLS – which they can't, because the server is not listening in plaintext mode on that port.

We will use a combination of automated tests for as much as possible and document the rest for manual or integration testing.

# 8. Operational Runbook (PCI-Grade) for mTLS

Even with robust automation, operations teams need clear procedures for certificate rotations, key compromises, and monitoring. Below is a runbook covering these scenarios:

# 8.1 Certificate Rotation Playbook

Normal Rotation (Expiration-based): Our certificates are short-lived (e.g. 1 hour via SPIRE or CSI). Rotation is automatic. However, we must ensure: - cert-manager (or SPIRE) is running properly to issue renewals. - trust-manager distributes any necessary trust changes.

Monitoring: Use the metrics we added: e.g., an alert if any service cert expires in $<$ N minutes. Alternatively, monitor cert-manager's Certificate resources for NotAfter times, or SPIRE Agent health.

If an alert triggers: 1. Identify affected service – e.g. Service X's certificate will expire in 10 minutes and has not been renewed. 2. Check cert-manager pods (or SPIRE agent) for issues. If cert-manager is down, restart it immediately. 3. Possibly trigger an immediate rotation: e.g., delete the CertificateRequest so cert-manager creates a new one, or in SPIRE trigger an SVID renewal via API. 4. In worst case (cert actually expires and service is now failing mTLS): as a quick fix, you can disable client auth on that service temporarily to restore some functionality (not great for security but might restore partial service – essentially fallback to plaintext or token auth if built-in). This requires a config change and restart to set mtls.enabled=false on that service, which is not ideal. A better recovery is to manually issue a cert: - Use the CA key to sign a new cert for that service, apply it via kubectl to the CSI volume (if possible). Or mount a secret with a temporary cert and adjust config to use it. - This is complex, so ideally we never hit expiry. 5. Once cert-manager is back, ensure new certificates issued, and services pick them up (our hot reload should handle it, or a quick pod restart if not). 6. Post-mortem: find out why rotation failed (Expired CA? cert-manager bug? etc.) and fix root cause.

Planned CA Rotation: Sometimes we need to rotate the root CA (e.g. about to expire, or routine security rollover). Steps: 1. Prepare new CA: Generate new root cert and key. Ideally, use an intermediate approach: e.g., keep a long-lived root offline, generate a new intermediate, let cert-manager use that. But for simplicity, new root. 2. Bundle Update (Phase 1): Using trust-manager, append the new CA to the trust bundle while keeping the old. Now all services trust both old and new. 3. Issue new certs: Configure the ClusterIssuer or SPIRE to start signing with the new CA. For cert-manager, that means updating the secret/ key that the ClusterIssuer uses to the new CA (and possibly keeping the old key to sign existing certificates until expiry - however cert-manager can only use one at a time, so we might have a short overlap where we trust both). - Alternatively, if using intermediate: have both intermediate A (old) and B (new) signed by the offline root, trust the root; then just start issuing with intermediate B. But if we didn't plan that, we do multi-root as above. 4. Reload trust: Trust-manager bundle now has both, which our services have loaded (hot reload sees bundle change and loads both CAs). 5. Phase 2 - migration: Now gradually, all new CertificateRequests will get signed by new CA. As pods rotate or renew, they get new CA. There is interoperability because everyone trusts both. 6. Monitor that, after some time (e.g., an hour if TTL=1h), no active certificates remain under old CA. We can check by scanning active pods' certificates. 7. Phase 3 - remove old CA: Once sure no old cert is in use (maybe wait a safety margin), remove the old CA from trust-manager sources and update bundle (trust-manager will update ConfigMap) 27 62 . Services reload and now only trust new CA. Any straggler with an old cert will fail now - ensure none left (coordinate with teams to restart anything missed). 8. Done. Document this change (especially if auditors need to know root CA changed on date X).

During this process, communication is key: coordinate with all service owners to ensure they don't panic if they see two CAs, and to restart any long-lived processes that don't auto-renew (shouldn't be any in our design).

# 8.2 Incident Response: Suspected Key Compromise

Scenario: We suspect a private key is compromised (e.g., an internal service's key leaked, or worst-case the CA key). The response depends on what was compromised:

- Workload (leaf) Private Key Compromise: For example, Service A's key is stolen. An attacker could potentially impersonate Service A until that cert expires. Steps:   
- Revoke credentials: If using SPIRE, we'd evict that SPIFFE ID (e.g., remove registration or ban the agent). If using cert-manager, since short lifetime is our main mitigator, we might manually invalidate that cert by removing the compromised pod (so it closes connections), and ensure no renewal. We can also add the cert's serial to a CRL if we maintained one (not currently). In absence of CRL, instruct all services via config update to add the compromised SPIFFE ID to a deny list (we could deploy a quick config that treats that specific ID as blocked). Our code doesn't have a denylist yet, but we can implement allowlist so not including that ID is effectively a deny.   
- Rotate Service Identity: Depending on threat, you might want to change Service A's SPIFFE ID (so that even if attacker has old cert, it won't be renewed under same ID or won't be allowed). This is advanced – means update service's identity in SPIRE or change service account in K8s (since SPIFFE ID often tied to SA). Possibly overkill; if key's stolen, just ensure it expires quickly and is not renewed to the attacker.   
- Notify other services: Use monitoring to detect if any unusual requests occurred from that ID (if we have audit logs of SPIFFE IDs on calls, check for anomalies).   
- In extreme case, shorten TTL globally to reduce risk window (but that increases load).   
- Post-incident, consider requiring that service to run in a more secure environment or with rotated credentials.   
- CA Key Compromise (Trust Anchor): This is critical. If the root CA is compromised, all issued certificates might be untrustworthy (attacker could mint their own). Response:   
- Immediately cease using the old CA. If SPIRE, rotate to a new root (SPIRE can rotate keys and propagate new trust bundles). If cert-manager, generate a new root and switch the ClusterIssuer to it.   
- Distribute the new CA as quickly as possible (trust-manager helps). Remove the old CA from trust to stop any attacker-issued certs from being accepted 63 64.   
- This effectively breaks all existing mTLS connections (since services won't trust their own old certificates once old CA removed). We must roll all pods to get new certificates under new CA quickly.   
- We might choose a brief overlap: trust both for a few minutes while everything re-issues, but if compromised, better to remove old immediately to stop bleeding – this will cause a full cluster mTLS outage until reissue, so it's a tough call. In a finance environment, likely we opt for immediate removal to stop any potential attacker usage, accepting a brief outage while everything rotates.   
- After rotation, carefully monitor for any further suspicious cert usage or traffic.   
- RCA: Determine how the key was compromised and fix that (HSM, tighter ACLs, etc.). Possibly audit all services for malicious cert usage in logs.   
SPIFFE ID Abuse without Key: e.g., an attacker somehow gets a workload to misuse its cert. In this case, the identities aren't compromised, but behavior is. That's more of an app-layer issue - outside

the scope of cert management (that's where additional authZ rules and monitoring come in, like unusual call patterns detection).

# 8.3 Revocation and Key Rotation Policies

We lean on short-lived certs to avoid heavy revocation processes. But if needed: - We can maintain a CRL for the CA: Vault can emit a CRL if a role is revoked, for instance. We can distribute CRLs via ConfigMap too, and use rustls's CRL support (currently rustls doesn't support CRL checking natively; we'd have to implement or use webpki's interface). - Instead, in SPIRE, one would use the "ban" feature or just wait the 1h until cert expires.

Key Rotation (regular): We should rotate the CA key at some fixed interval (e.g., annually or semi-annually) even if not compromised, as a hygiene practice. That process is covered in CA rotation above. Also, rotate SPIRE server keys similarly if using SPIRE. Workload keys are rotated automatically (1h TTL means new key every hour essentially).

Service Redeployments: Ensure when a service pod terminates, its cert is not reused. CSI driver mounts keys in memory and removes on pod deletion 65, so that's good. If someone somehow grabs the key from a dead pod's node memory, mitigated by short TTL.

# 8.4 Monitoring & Detection

# - Prometheus Alerts:

- Alert if any brrt_tls_handshake failures_total spikes or has a sustained non-zero rate (could indicate a misconfigured client or an attacker scanning).   
- Alert on brrt_tls_authz-denials_total > 0 (someone tried to connect with a valid cert but not allowed - could mean either a config issue or an intrusion attempt).   
- Alert if cert expiry days < 1 for any service (if exposed as gauge).   
- Also integrate with cert-manager metrics if possible (it has metrics for cert issuance errors).   
- If using SPIRE, monitor SPIRE Server/Agent health and logs (they can emit events if a workload isn't attested, etc.).   
- Log Analysis: Feed logs into ELK/Datadog etc. Specifically flag warnings from our mTLS layer:   
- Multiple "SPIFFE ID not allowed" warnings - investigate source IP, ID, etc.   
- Any "certificate expired" or handshake errors beyond a threshold – investigate if an outage is looming.   
- Also log the client SPIFFE ID in access logs of BRTRouter for traceability (like include it in the HTTP access log line or tracing span). This allows auditing calls by identity, which is crucial for compliance (e.g., you can answer "which services called this endpoint in the last month" by filtering logs by SPIFFE ID).   
- Penetration Testing: Periodically, run internal penetration tests:   
- Try to use a cert from another environment (should fail trust domain check).

- Try to use an expired or not-yet-valid cert (our system should catch notBefore as well with leeway) 66 67.   
- Attempt to MITM traffic (should be impossible due to mTLS, but testers might try to present a fake CA – our trust anchor restricts that).   
- Ensure no fallback to plaintext: e.g., ensure no service has an open plain HTTP port by mistake.   
- Documentation & Training: Document the procedures in internal wiki so that on-call engineers know how to handle an alert at 3am about "mTLS handshake failures" - e.g., check if a CA rotated or if an unauthorized call is happening. Also document how to add a new service's SPIFFE ID to allowlist if needed (with proper change control).

By following these runbook steps, the team can confidently maintain the mTLS infrastructure with high security. Regularly test these runbook steps in staging (simulate a CA rotation rehearsal, etc.) to ensure we're prepared. This level of rigor is in line with PCI/SoX compliance: everything is auditable (we can show logs of identities), controlled (only our CA issues certs), and we have rapid response for incidents.

# 9. Recommended Default Blueprint (Config & Code Examples)

Finally, to help adoption, here's a blueprint of manifests and code snippets to implement the above in a Kubernetes environment with cert-manager:

# 9.1 Kubernetes Manifests for CA and Trust

# ClusterIssuer for Internal CA:

```yaml
apiVersion: cert-manager.io/v1  
kind: ClusterIssuer  
metadata:  
    name: spiffe-ca-issuer  
spec:  
    ca:  
        secretName: spiffe-ca-root # This secret holds our CA key & cert 
```

(Create the spiffe-ca-root secret beforehand with keys tls.crt and tls.key. For production, generate this with a secure method and keep the key safe. Optionally, use cert-manager create ca -- name spiffe-ca-root ... utility).

SPIFFE CSI Driver + Approver: Install via Helm (from Jetstack):

```shell
helm install cert-manager-csi cert-manager/csi-driver-spiffe \
--namespace cert-manager --set issuerRef.name=spiffe-ca-issuer 
```

```batch
--set issuerRef.kind=ClusterIssuer --set nodeSelector="kubernetes.io/os: linux" 
```

(This installs the CSI driver DaemonSet and the approver Deployment. Values ensure it knows which issuer to use.)

trust-manager Bundle:   
```yaml
apiVersion: trust_cert-manager.io/v1alpha1  
kind: Bundle  
metadata:  
    name: spiffe-trust-bundle  
spec:  
    sources:  
        - secret:  
            namespace: cert-manager  
            name: spiffe-ca-root  
            key: TLS.crt  
        target:  
            configMap:  
                name: spiffe-trust-bundle  
                key: ca:bundle.pem  
            # target namespace (by default, trust-manager writes to a special namespace or we can specify one). 
```

We might deploy trust-manager itself via Helm as well. After applying this Bundle, trust-manager will create the ConfigMap spiffe-trust-bundle (likely in cert-manager ns or "trust" ns if configured) with our root cert.

Service Deployment Example: For each service (including BRRTRouter gateway if internal), update the Deployment:

```yaml
spec: template: metadata: annotations: # Ensure service account token in audience for CSI (if required; latest driver uses projection) # ... spec: serviceAccountName: my-service-sa volumes: - name: spiffe-credits csi: driver: spiffe.csi.cert-manager.io 
```

```yaml
readOnly: true  
volumeAttributes:  
    csi.cert-manager.io/issuer-name: "spiffe-ca-issuer"  
    csi.cert-manager.io/issuer-kind: "ClusterIssuer"  
    csi.cert-manager.io/issuer-group: "cert-manager.io"  
    csi.cert-manager.io/trust-domain: "rerp.prod"  
- name: trust-bundle  
configMap:  
    name: spiffe-trust-bundle # created by trust-manager  
    items:  
        - key: ca.bundle.pem  
            path: ca.crt  
containers:  
- name: app  
    image: my-service:latest  
    volumeMounts:  
    - name: spiffe-credits  
        mountPath: /var/run/secrets/spiffe.io  
        readOnly: true  
    - name: trust-bundle  
        mountPath: /etc/spiffe  
        readOnly: true  
env:  
    - name: BRTR_MTLS_ENABLED  
        value: "true"  
    - name: BRTR_MTLS_cert_FILE  
        value: "/var/run/secrets/spiffe.io/tls.crt"  
    - name: BRTR_MTLS_KEY_FILE  
        value: "/var/run/secrets/spiffe.io/tls.key"  
    - name: BRTR_MTLS_CA_FILE  
        value: "/etc/spiffe/ca.crt"  
    - name: BRTR_MTLS_TRUST_DOMAIN  
        value: "rerp.prod"  
# Optionally, allowed client IDs pattern:  
- name: BRTR_MTLS ALLOWED_IDS  
    value: "spiffe://rerp.prod/ns/frontend/*" 
```

The above ensures the pod gets its own cert and the trust bundle. We configure the app via env (BRTR stands for BRTRouter) to use those files. The application reads these in AppConfig.security(mtls) (we map env to config fields as shown in main.rs with clap or env parsing). This example allows any ID from namespace "frontend" to call it.

Note: Each service should ideally run under a distinct service account (to have distinct SPIFFE IDs). We use that in the volume attributes implicitly: the CSI driver will fill the CSR URI with the service account and namespace.

# 9.2 BRRTRouter Configuration Example

In code (Rust), the AppConfig might look like:

```rust
#[derive(Deserialization)]   
struct MtlsConfig{ enabled: bool, cert_file: String, key_file: String, caBundle_file: String, trust_domain: Option<String>, allowed_id�建者：Option<Vec<String>>， }   
#[derive(Deserialization)]   
struct SecurityConfig{ //...existing fields mtls:Option<MtlsConfig>， 
```

This corresponds to the YAML/env we set above. The BRTR_MTLS ALLOWED_IDS can be a comma-separated list that we split into Vec.

At runtime, after loading config:

```txt
if let Some(mtls) = config.security(mtls && mtls.enabled {
// Configure rustls as described in Implementation Plan
lettls_server = TlsServer::new(socket, &mtls)?;
tls_server.start(); // blocking accept loop with coroutines
} else {
HttpServer(socket.service).start(addr)?..join());
} 
```

(This is a simplification. Actually, we integrate with may's coroutine scheduling as described.)

# 9.3 Minimal Rust mTLS Example

For illustration, here's a simplified snippet using rustls for mutual TLS with SPIFFE:

```rust
use rustls::{ServerConfig, ClientConfig, Certificate, PrivateKey, RootCertStore, AllowAnyAuthenticationClient};
use std::fs;
use x509_parser::parse_x509_certificate;
use webpki::DNSNameRef; 
```

```rust
// Load server cert and key
let cert_chain = fs::read("/var/run/secrets/spiffe.io/tls.crt").unwrap();
let key_bytes = fs::read("/var/run/secrets/spiffe.io/tls.key").unwrap();
let certs = rustls.pemfile::cents(&mut cert_chain.as Slice().unwrap())
    .into_iter().map(Certificate).collect();
let mut keys = rustls.pemfile::pkcs8_private_keys(&mut
key_bytes.as Slice().unwrap());
let priv_key = PrivateKeykeys.remove(0);
// Load client trust anchors (our root CA)
let ca_bytes = fs::read("/etc/spiffe/ca.crt").unwrap();
let mut rootstore = RootCertStore::empty();
rootstore.add_pem_file(&mut ca_bytes.as Slice().unwrap());
let client_auth = AllowAnyAuthenticatedClient::new(rootstore.clone());
// Server config
let mut server_config = ServerConfig::new(client.auth);
server_config.setsingle_cert(certs, priv_key).unwrap();
server_config-certVerifier = Arc::new(MyServerCertVerifier{ roots:
root_store.clone(), trust_domain: Some("erp.prod".to_string())});
// (MyServerCertVerifier would implement rustls::ServerCertVerifier to check
client cert URI SAN)
// Client config (for a client making requests)
let mut client_config = ClientConfig::new();
client_config.rootstore = root/store;
client_config.set_single_client_cert(certs.clnone(), priv_key.clnone()).unwrap();
client_config.dangerous().set_certificates.Verifyier(Arc::new(MyClientCertVerifier
{ roots: rootstore, expected_domain: "erp.prod".to_string()})};
// (MyClientCertVerifier implements rustls::ServerCertVerifier, checking server
cert's SPIFFE URI has trust_domain "erp.prod")
// Example of extracting SPIFFE from a cert using x509-parser:
fn spiffe_from_cert(cert: &Certificate) -> Option<String> {
    let res = parse_x509_certificate(&cert.0);
    if let Ok((_, rem, cert)) = res {
        for ext in cert.extensions() {
            if ext.oid == OID_EXTERNAL_NAME { // pseudo-code OID for SAN
            if let ParsingExtension::SubjectAlternativeName(san) =
            ext.parsed_extension() {
                for name in san.general_names {
                    if let GeneralName::URI(uri) = name {
                        if ur.starts_with("spiffe://") {
                            return Some-uri.to_string());
                        }
                        }<nl> 
```

```txt
} } } None } 
```

In the above: - We configured server to require client auth via AllowAnyAuthenticationClient(rootstore) - this ensures the client cert must chain to our CA 19. - We set a custom verifier for server to verify client's cert (technically AllowAnyAuthenticationClient already does chain verification, but we might need to add SPIFFE ID check, which we can do post-handshake or by customizing that flow). - The client config uses set_single_client_cert to load its own cert to present, and a custom verifier to check the server. - The spiffe_from_cert shows how to parse the certificate and find the URI SAN (x509-parser yields it; we'd integrate similar logic in our verifiers or after handshake). - DNSNameRef is not used because we skip DNS verification in lieu of SPIFFE.

# Allowed/Blocked Decision Example:

```rust
// Pseudocode inside handshake handling
if let Some(peer_certs) = tls_session.peer_certificates() {
let peer_cert = &peer_certs[0];
if let Some-uri) = spiffe_from_cert(peer_cert) {
if !uri.starts_with("spiffe://rrp.prod/"）{
println("Reject: wrong trust domain {},uri);
//abort connection
} else if let Some(patterns) = allowedpatterns {
let ok = patterns.iterator().any(|p|matches_pattern(p,&uri));
if !ok {
println("Reject:{}not in allowed list",uri);
//abort
}
println("Accepted client ID:{}",uri);
} else {
println("Reject: no SPIFFE URI in cert");
//abort
}
} 
```

Where matches_pattern implements wildcard matching (as described earlier).

This blueprint provides a concrete starting point. In real code, we'll handle errors appropriately, integrate with BRTRouter's async model, and ensure everything is thread-safe (rusts config are Arc, etc.).

By following this design and using the provided examples, we will achieve a robust SPIFFE-based mTLS security for all service-to-service communication. Each request will be mutually authenticated with strong cryptographic identity, meeting the high bar required for financial-grade, zero-trust infrastructure 10 6

# Sources:

SPIFFE spec and concepts 3 4 6   
- BRTRRouter code analysis 7 29   
- cert-manager SPIFFE CSI docs 68 19   
- Istio SPIFFE patterns 14   
- Let's Encrypt rate limit documentation 33 34

1 2 3 4 5 6 8 9 10 13 SPIFFE | SPIFFE Concepts

https://spiffe.io/docs/latest/spiffe-about/spiffe-concepts/

7 42 mod.rs

https://github.com/microscalerr/BRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/src/security/spiffe/mod.rs

11 12 14 15 18 38 Istio / SPIRE

https://istio.io/latest/docs/ops/integrations/spire/

16 17 29 30 43 44 45 46 47 48 49 50 51 52 53 54 66 67 validation.rs

https://github.com/microScaler/BRRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/src/security/spiffe/ validation.rs

19 20 21 22 23 24 31 32 37 41 65 68 csi-driver-spiffe - cert-manager Documentation

https://cert-manager.io/docs/usage/csi-driver-spiffe/

25 26 27 28 35 36 39 40 62 63 64 trust-manager - cert-manager Documentation

https://cert-manager.io/docs/trust/trust-manager/

33 34 Rate Limits - Let's Encrypt

https://letsencrypt.org/docs/rate-limits/

55 56 57 58 main.rs

https://github.com/microcaler/erp/blob/d9800433f9aab3298de2a86637e5b716e6d94463/microservices/accounting/bff/gen/src/main.rs

59 Cargo.toml

https://github.com/microScaler/BRRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/Cargo.toml

60 README.md

https://github.com/microcaler/BRRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/README.md

61 spiffe/tests.rs

https://github.com/microscalerr/BRRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/tests/spiffe/tests.rs
