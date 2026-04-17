# SPIFFE/SPIRE Mutual TLS Architecture for BRRTRouter Services

# 1. SPIFFE/SPIRE Primer: Identities & SVIDs in Zero-Trust

SPIFFE (Secure Production Identity Framework for Everyone) is an open standard for authenticating software services in dynamic environments using strong identities. A SPIFFE ID is a URI of the form spiffe://<trust-domain>/<path>, which uniquely identifies a workload 1 2. For example, spiffe://finance.local/payments/serviceA could identify service A in the payments system. The trust domain (e.g. finance.local) is the root of trust - all identities in that domain are issued and verifiable by the same authority 3.

SPIFFE Verifiable Identity Documents (SVIDs) are the cryptographic identity credentials containing a SPIFFE ID $4$ . There are two types of SVIDs: - X.509 SVID - an X.509 certificate (and private key) containing the SPIFFE ID in the URI Subject Alternative Name (SAN). Used for mutual TLS. - JWT SVID - a JWT token carrying the SPIFFE ID in its claims (e.g. sub claim) $5$ .

X.509 SVIDs are short-lived certificates (typically hours) automatically rotated to mitigate key compromise 6 7 . They are primarily used for mTLS so that two services can authenticate each other's SPIFFE ID during the TLS handshake 8 . X.509 SVIDs are preferred whenever possible because JWT tokens can be replayed if intercepted 8 . As the SPIFFE docs note, use X.509-SVIDs whenever possible – JWT-SVIDs are mainly for cases where mTLS isn't feasible (e.g. an HTTP layer 7 proxy that can't forward client certs) 9 .

JWT SVIDs are JWT tokens signed by the trust domain authority, including the SPIFFE ID as the subject (sub) and intended audiences (aud). They allow bearer-token style authentication. However, because they are bearer tokens (susceptible to theft and replay), they should have very short lifetimes and are best used only where X.509 mTLS can't be applied 8 .

Workload attestation vs. cert distribution: A key SPIFFE tenet is that workloads don't manually manage certificates or secrets. Instead, a Workload API (typically provided by a SPIRE Agent) automatically issues SVIDs to an attested workload 10 11 . In a SPIFFE/SPIRE deployment, each node runs an agent that authenticates to a SPIRE server (e.g. via node attestation like AWS IID, Kubernetes token, etc.), and each workload process proves its identity (via selectors like pod labels, service account, etc.) to get its SVID 12 13 . This attestation-based issuance means no static sharing of CA keys or long-lived certificates - workloads get ephemeral, least-privilege credentials at runtime. In contrast, a traditional "cert distribution" approach (manually pre-generating certificates or mounting secrets) is more error-prone and less secure: it lacks automatic rotation, and a compromised cert could be reused elsewhere. SPIFFE's approach ensures that identities are tied to actual running workloads (e.g. a pod with specific properties) and that credentials are short-lived and rotated without operator intervention 6 . This underpins zero trust: no machine or network is implicitly trusted - each service must present a valid SVID and prove who it is on every connection.

SPIRE (SPIFFE Runtime Environment) is the CNCF project implementing SPIFFE. A SPIRE deployment consists of a central SPIRE Server (certificate authority for the trust domain) and SPIRE Agents on each node 14 15 . The server issues SVIDs after verifying node and workload attestation, and agents expose a local Workload API socket for applications to fetch their SVIDs 15 . SPIRE automates key tasks like: - Generating a trust bundle (CA root certificates) for the trust domain 16 . - Automatically signing and rotating SVID certificates and tokens. - Distributing the trust bundle to workloads so they can validate each other 17 18 . - Optionally federating trust across domains (SPIFFE Federation).

In summary, SPIFFE provides strong cryptographic identities (SVIDs) bound to workloads, and SPIRE automates their lifecycle via attestation. This creates a foundation for zero-trust service-to-service mTLS - every API call is mutually authenticated at the connection layer, eliminating implicit trust in network identity (like IPs) and drastically reducing the blast radius of credential leaks or MITM attacks.

# 2. Workload Identity & mTLS Architecture Options for BRRTRouter

BRRTRouter-based microservices need a robust service-to-service authentication architecture. We consider three viable options to achieve SPIFFE-compliant mTLS, each with trade-offs in complexity, operational overhead, and compatibility:

# Option 1: Native SPIRE Infrastructure (SPIFFE IDs via SPIRE Server & Agents)

Architecture: Deploy SPIRE Server as an internal CA and SPIRE Agents on every node (in Kubernetes, as a DaemonSet). Each service (including BRRTRouter gateway and all internal services) uses the SPIRE Workload API to obtain its X.509 SVID and trust bundle. All service-to-service communication (REST calls, gRPC calls) is done over mTLS using these SVIDs.

- Trust Domain Design: Define a trust domain for your environment, e.g. spiffe://prod.acme.com. Within this, SPIRE can issue identities that incorporate Kubernetes metadata. A common convention is to include namespace and service account: e.g. spiffe://prod.acme.com/ns/<namespace>/sa/<serviceAccount> 19. For example, the "orders" service in namespace "frontend" running as serviceAccount "orders-sa" might get spiffe://prod.acme.com/ns/backend/sa/orders-sa. SPIRE supports flexible ID templates, but using the K8s SA identity in the SPIFFE ID makes it easy to reason about which Kubernetes service corresponds to which SPIFFE ID 20.   
- Identity Issuance & Attestation: When a pod running service $A$ starts, the SPIRE agent on that node attests it (e.g. checks it's running under the expected service account or with certain labels) and requests an SVID from SPIRE Server $^{12}$ . The server, if the pod matches a registered identity, issues an X.509 SVID certificate (say for $\underline{\dots}/\text{sa/orders-sa}$ ) and the agent delivers it (and the trust bundle) to the service via the Workload API $^{21}$ . Service $A$ stores the cert and key (often just in memory, or via a SPIRE client library). Similarly, service $B$ gets its own SVID (e.g. $\underline{\dots}/\text{sa/payments-sa}$ ).   
- mTLS Handshake & Identity Verification: When service A calls service B over TLS, both present their SVIDs. Because both trust the same trust bundle (SPIRE's CA), the TLS handshake succeeds only if each presents a certificate signed by the SPIRE CA $^{23}$ . Peer SPIFFE IDs are extracted

from the cert URI SANs on each side. BRRTRouter (or underlying TLS libraries like rustls) must verify that the presented certificate's SPIFFE ID matches an allowed trust domain (e.g. prod.acme.com) and is properly signed by the trust anchor. By default, SPIRE's CA is private – no public CA is involved, eliminating external dependencies.

- Preventing Spoofing: Since only the SPIRE server (which is secured and attested itself) can issue SVIDs for prod.acme.com, an attacker cannot fabricate a valid certificate for that trust domain without compromising SPIRE. Workload attestation ensures that even if an attacker got access to the cluster, they cannot obtain an SVID for a service they shouldn't (e.g. they can't get an orders-sa SVID unless they compromise that workload's identity or the SPIRE server). The SPIFFE verifier in BRTRouter should enforce that the client certificate's SPIFFE URI exactly matches an expected pattern or allowed list if you want an additional authorization layer. For example, Service B might only accept calls from identities with prefix spiffe://prod.acme.com/ns/backend/sa/ (i.e. only backend namespace services), or an explicit list of IDs. This can be achieved by checking the URI SAN after TLS handshake. (We will cover BRTRouter implementation details later.)

- Rotation & Revocation: SPIRE issues short-lived certificates (by default often 1 hour or similar, configurable). Agents will proactively renew SVIDs before expiry, so workloads get seamless rotation. BRRTRouter should be able to reload its credentials without restart - in the SPIRE model, the SPIRE Agent or SPIFFE Workload API provides fresh certificates in-memory. If a service's credentials are suspected compromised, SPIRE can ban that workload (so it no longer receives SVIDs) and optionally rotate the CA (which would invalidate all existing SVIDs, requiring restart of connections - an extreme measure). SPIRE also supports JWT-SVID revocation lists and X.509 revocation via short lifetimes (revocation is usually handled by just not renewing a compromised identity).

Trade-offs: Option 1 provides the strongest security and aligns $100\%$ with SPIFFE standards. It is suitable for high-security environments (finance-grade). It introduces operational overhead - running SPIRE server (which must be highly available) and agents, and managing registration entries for workloads. However, it is highly scalable and automated. It's the only option that covers non-Kubernetes workloads easily (SPIRE has attestors for VMs, AWS, etc.). If multi-cluster or multi-cloud is in play, SPIFFE Federation can connect trust domains or bridge multiple SPIRE servers. This option avoids any dependency on external PKI or internet connectivity. The downside is the learning curve and managing another piece of infra (SPIRE), but given the importance of identity, this is often justified.

# Option 2: Kubernetes cert-manager with Internal CA & SPIFFE-Like Identity

Architecture: Use Kubernetes cert-manager as an internal certificate management system. We configure a ClusterIssuer backed by an internal CA (or an intermediate from an internal PKI, or HashiCorp Vault PKI). Each service gets an X.509 certificate from this CA. We encode the SPIFFE ID in the certificate (as a URI SAN), even though issuance is via cert-manager. Essentially, we mimic SPIFFE's identity semantics using cert-manager's mechanisms. This can be combined with the cert-manager SPIFFE CSI driver for automatic key/ cert provisioning to pods.

- Trust Domain and CA: Decide on a trust domain (e.g. spiffe://prod.acme.com). We'll create a Root CA for this domain (or use an existing internal CA) and install it in the cluster (e.g. as a secret for cert-manager). This CA is not public - it's dedicated to internal mTLS. In cert-manager, one approach is defining a CA Issuer with that root. Alternatively, use Vault as an Issuer (Vault can act as a CA) or

Smallstep's step-ca via step-issuer. In all cases, services will trust only this CA for internal communications.

- Identity Encoding: To get SPIFFE IDs on cents, we have two sub-options:   
(2A) Manual Certificate specs: For each service, define a Certificate custom resource that requests a certificate for the service's SPIFFE URI. For example, for service A in namespace "frontend":

```yaml
apiVersion: cert-manager.io/v1  
kind: Certificate  
metadata:  
    name: service-a-cert  
    namespace: backend  
spec:  
    secretName: service-a-tls  
    duration: 1h  
    renewBefore: 30m  
    issuerRef:  
        name: internal-ca-issuer  
        kind: ClusterIssuer  
    commonName: "service-a" # (commonName can be left empty or a human-readable name; SPIFFE uses SAN)  
    dnsNames: [ ] # no DNS SANs for internal use  
    urisANs:  
        - "spiffe://prod.acme.com/ns/dashboard/sa/service-a-sa"  
    usages: ["client auth", "server auth"] 
```

This certificate will contain the SPIFFE URI in the SAN (and no other SANs) and will be valid for mTLS (Key Usage: digitalSignature; EKU: clientAuth, serverAuth). The cert-manager CA Issuer signs it. We would do this for each service (perhaps automated with a template or Helm chart for consistency).

- (2B) Automated (CSI driver): A better approach is using cert-manager's CSI driver for SPIFFE (csi-driver-spiffe). This plugin automatically requests a cert for the pod's service account identity and mounts it. When a pod with the CSI volume starts, the driver generates a key and creates a CertificateRequest to the issuer with:

A URI SAN of spiffe://<trust-domain>/ns/<namespace>/sa/<serviceAccount>(it derives this from the pod's service account) 25.   
Key usages of clientAuth and serverAuth 25.   
A short default duration (e.g. 1 hour) 25   
No other SANs or extraneous fields (ensuring the cert is exclusively an SPIFFE ID) 25.

The CSI driver then mounts the issued cert and key into the pod's filesystem (usually at /var/run/ secrets/spiffe.io). It keeps them updated (monitoring the cert's expiry and renewing via certmanager as needed) 26 27. This gives each pod a unique, automatically rotated SVID without

running SPIRE. The private key never leaves the node's memory and is not shared with the K8s API (only the CSR is) 27 .

Example: Service A's Deployment spec would include:

```yaml
volumes:  
- name: spiffe-credits  
    CSI:  
        driver: spiffe.csi.cert-manager.io  
        readOnly: true  
        volumeAttributes:  
            trustDomain: "prod.acme.com"  
volumeMounts:  
- name: spiffe-credits  
    mountPath: "/var/run/Secrets/spiffe.io"  
    readOnly: true 
```

The SPIFFE CSI driver ensures the pod gets a cert with URI SAN spiffe://prod.acme.com/ns/ backend/sa/service-a-sa and the trust bundle (CA cert) all mounted in that directory.

- mTLS Handshake & Verification: Similar to Option1, all services trust the internal CA. BRRTRouter and other services must be configured to require client certificates and trust only the internal CA's cert (or its root). The trust bundle can be distributed via a ConfigMap or using cert-manager's trust-manager to combine and distribute CA certificates cluster-wide 28 29 (the trust-manager's Bundle CRD could package the internal CA and any others needed). Each service, at startup, loads the CA bundle (from file or ConfigMap) into its TLS stack. When Service A connects to Service B:   
- The server (B) checks the client cert is signed by the CA and extracts the URI SAN. It should verify the URI starts with spiffe://prod.acme.com (trust domain) and optionally check that it matches an allowed service identity (if B wants to only allow specific callers).   
- The client (A) verifies B's server cert against the CA and that the URI SAN is in the trust domain and corresponds to the expected server identity (in some cases, the client might know it's calling B specifically and can enforce the exact SPIFFE ID of B).   
- Spoofing prevention: As long as the CA is private and under our control, an attacker cannot get a cert for our trust domain unless they compromise cert-manager or the CA. However, note that cert-manager by itself doesn't do workload attestation - any pod that can create a CertificateRequest with the correct URI could theoretically get a cert. The CSI driver mitigates this by ensuring the URI SAN in the request must match the pod's service account and requiring an approver that checks this. This means a compromised pod can only obtain a cert for its own identity, not impersonate another service. We should enforce that only the CSI driver's requests (with the special annotation) are approved, to avoid developers accidentally requesting arbitrary SPIFFE IDs.   
- Rotation & Revocation: With cert-manager, we can configure short certificate durations (e.g. 1 hour as above). cert-manager will normally attempt renewal renewBefore (e.g. 30 minutes before expiry). The CSI driver handles triggering new CertificateRequests and updating the files in the pod

volume 26. BRRTRouter (and any service) needs to handle cert/key reloading. If using the CSI volume approach, the cert and key file on disk will change on rotation; the application can either periodically refresh its TLS config or, better, use something like watching the file or using a rustls ResolvesServerCert that picks up new certs. For client-side, if using something like rustls native TLS, it might require re-reading the client cert. A simpler approach is to have the process automatically restart on certificate rotation (but that could cause a blip). Ideally, implement hot reload: e.g. BRRTRouter could watch the cert file (using the existing notify crate already in dependencies) and update its in-memory TLS config when the file changes.

Revocation in this model is mostly handled by short lifetimes. If a key is compromised, one could manually revoke via CRL, but cert-manager CA Issuer does not automatically publish CRLs. Instead, you'd remove the compromised pod (so it no longer renews) and possibly rotate the CA if needed. If using Vault, Vault can revoke specific certificates by serial number (and maintain CRL distribution which our services could check). However, checking CRLs in every service might be complex and is usually avoided in favor of short-lived certificates. Overall, the design assumes ephemeral credentials limit the window of misuse. For catastrophic compromise (e.g. CA key leak), you'd do a trust bundle rotation (deploy a new CA and update all workloads to trust new CA while phasing out the old).

Trade-offs: Option 2 can achieve the same end-state (every service has a SPIFFE-like URI identity and mTLS) without deploying SPIRE, using Kubernetes-native tools. It's less full-featured in attestation (mostly ties identity to service account names, which is usually okay for K8s). It's easier to adopt if you already use cert-manager for ingress certificates. The CSI driver approach, in particular, brings automation closer to SPIRE's convenience. This option is Kubernetes-centric: it doesn't automatically cover VMs or non-K8s components (you'd need separate workflows for those - e.g. running cert-manager on VMs or using Vault). It also relies on the security of the K8s control plane (which is generally strong, but cert-manager's cert issuance should be locked down by RBAC so that only the CSI driver or controlled entities can request SPIFFE certificates, otherwise a compromised component could request a cert for another service's URI if not careful).

Compared to SPIRE, cert-manager + CSI is less complex operationally (no separate server with plugins), but also less flexible (doesn't have fancy attestors - basically uses the Kubernetes service account token as its attestation via the CSI driver token request $^{30}$ $^{25}$ ). If your needs are entirely within Kubernetes, this is a solid approach. It keeps everything in the K8s API ecosystem and leverages the robust cert-manager project (CNCF graduated) for CA management.

# Option 3: Service Mesh (Istio/Linkerd) with Built-in SPIFFE IDs

Architecture: Introduce a service mesh that automatically secures all pod-to-pod traffic with mTLS. Both Istio and Linkerd implement mTLS with identities that either conform to the SPIFFE ID format or are very similar. Istio, for example, issues certificates with SPIFFE URIs like spiffe://cluster.local/ns/ <namespace>/sa/<serviceaccount> by default (Istio's Citadel / Istiod acts as a CA) 31 . Linkerd has recently added first-class SPIFFE support as well 32 (older Linkerd versions used a similar format without the spiffe:// prefix, but now they can integrate with SPIFFE IDs too).

In a mesh scenario, every service pod gets a sidecar proxy (e.g. Envoy in Istio or Linkerd2-proxy in Linkerd) which handles TLS. The sidecar is provisioned with a certificate (automatically by the mesh control plane)

representing the service's identity. All traffic between sidecars is transparently mTLS. This means services don't need to implement TLS at the application layer – the proxies do it.

- Trust Domain: By default Istio uses cluster.local as the trust domain (configurable). It effectively operates its own SPIFFE trust root. You would set the mesh's trust domain to something like prod.acme.com to align with SPIFFE. Istio Citadel can also integrate with external CAs or Vault if needed. Linkerd by default uses a root you provide (via linkerd install you supply or generate trust anchor and issuer certs). If using Linkerd's SPIFFE support, you'd ensure the trust domain is set appropriately.   
- Identity Naming: Istio's identities are spiffe://<trust-domain>/ns/<ns>/<sa><sa> which aligns with SPIFFE spec conventions 33. Linkerd's default identity is similar (Linkerd 2.11: foo.bar.serviceaccount.identity(linkerd.cluster.local) inside the cert CN, but with SPIFFE support it can use standard SPIFFE URIs). The key point is the mesh ensures that if service A talks to service B, B's proxy sees A's service account and namespace in the certificate identity.   
- BRRTRouter Integration: If BRRTRouter is the front-door API gateway, it would also run with a sidecar (or be mesh-injected). The gateway's incoming external traffic might terminate TLS at the proxy or at BRRTRouter itself – typically, you'd still let BRRTRouter handle the external TLS with Let's Encrypt (since it needs to serve HTTP), but the internal sidecar-to-sidecar traffic would be mTLS. The BRRTRouter service, when calling internal downstream services, would go out through its sidecar, automatically secured.

Because the mesh handles the mTLS, BRRTRouter's code doesn't need to use rustls for mTLS - it would just make plaintext calls to "service B" (as the sidecars encrypt on the wire). However, that means BRRTRouter itself wouldn't directly inspect SPIFFE IDs unless you use mesh APIs to get that info (or implement authorization policies in the mesh layer). Istio and Linkerd allow setting policies: e.g. "Service B only allows incoming from identity X". These policies are configured at the mesh level (Istio AuthorizationPolicy, Linkerd Server Authorization CRD, etc.) rather than in the application code.

- Preventing Spoofing: The mesh ensures no traffic goes unencrypted or unauthenticated between meshed services. Istio's Envoy proxies won't accept a connection from a workload that doesn't present a valid cert issued by the mesh's CA. So an outsider or rogue pod without a sidecar can't communicate with a meshed service over the service mesh's port. This greatly reduces spoofing risk inside the cluster. However, one must ensure that all service ports are covered by the mesh (or else an attacker might hit a non-mTLS port). Usually, one would mandate that all inter-service communication goes through the mesh.   
- Rotation & Revocation: Mesh control planes handle rotation of the sidecar certificates (Istio's default is 90-day cents, renewed ~45 days in; Linkerd's default is 24 hours). The proxies automatically fetch new cents via the control plane. The trust anchors can be rotated by updating the mesh configuration (though it can be non-trivial to do a seamless trust root rotation in a mesh, it's supported via overlapping roots in newer versions). If a workload's key is compromised, you can ban the workload (e.g. remove its sidecar or label so it's not in mesh) – but as long as the key is short-lived, it will expire soon. Mesh control planes currently don't distribute CRLs; they rely on short TTLs for credentials and on the assumption that if a pod is compromised it will be ejected.

Trade-offs: Using a service mesh is the least amount of application development effort – BRRTRouter and other services wouldn’t need to change code to enforce mTLS. It is effectively “mTLS by default.” Additionally, meshes bring other features (observability, traffic management). However, a mesh is a heavyweight addition: it adds sidecar overhead (CPU/memory), complexity in operations (injection, debugging sidecars, version compatibility), and another control plane to manage. In latency-sensitive or resource-constrained environments, a full mesh might be undesirable. Also, if your architecture spans beyond K8s (e.g., some VMs), Istio can include VMs with some work, Linkerd 2.12+ can handle VMs with their “mesh expansion” using SPIFFE federation or manual cert provisioning [34], but it’s additional complexity.

Given the context ("meshless environments" were mentioned), Option 3 might not be the first choice here, but it's worth noting. If one already has Istio/Linkerd, leveraging it for identity is straightforward. If not, Option 1 or 2 are more lightweight for the singular purpose of mTLS.

Diagram - Trust Architecture & Identity Flow: The following diagram illustrates a possible combination of these ideas. It shows an external client connecting via BRRTRouter (with a public cert) and internal services using SPIFFE-based mTLS. The trust domain [example.com] is used internally, distinct from public CA trust.

```batch
flowchart LR  
subgraph Internal Trust Domain (spiffe://example.com)  
direction TB  
CA[(SPIRE Server / Internal CA)]::trust ->|issues SVID| SvcA[Service A Pod]:::workload  
CA[(SPIRE Server / Internal CA)]::trust ->|issues SVID| SvcB[Service B Pod]:::workload  
SvcA == mTLS (SPIFFE auth) => SvcB  
SvcA ---::hidden SvcA %% dummy link to force subgraph box  
end  
classDef trust fill:#e6ffe6,stroke:#88cc88;  
classDef workload fill:#FFFFFF,stroke:#333,stroke-width:1px;  
classDef hidden stroke-width:0px;  
subgraph External World  
direction TB  
Client[External Client]:::ext  
end  
classDef ext fill:#ffefd5,stroke:#cc8844;  
subgraph Cluster Ingress  
Gateway[BRRTRouter Gateway]:::gateway  
classDef gateway fill:#e6f7ff,stroke:#33a;  
end  
Client -- HTTPS (Public CA cert) -> Gateway  
Gateway -- mTLS (SPIFFE cert) -> SvcA 
```

(The diagram above shows an external client connecting to BRRTRouter's gateway using a public TLS certificate (e.g. from Let's Encrypt), while inside the cluster all service-to-service calls use mutual TLS with SPIFFE-issued

certificates. The trust boundary is such that the internal CA (SPIRE or cert-manager CA) is separate from public PKI.)

# 3. Certificate Issuance, Rotation, and Trust Bundle Management (cert-manager Strategy)

Regardless of whether Option 1 or 2 is chosen, a robust certificate management strategy is needed. Here we focus on Kubernetes deployments with cert-manager, since it will be present even if SPIRE is used (you may still use cert-manager for ingress or other certs). Key considerations:

# Issuer and CA Choices for Internal mTLS

For internal certificates, do NOT use a public CA (like Let's Encrypt) – instead, use an internal CA that you control. This internal CA can be set up in several ways: - cert-manager CA Issuer: Easiest path. You create your own root CA (e.g. using OpenSSL or cfsssl) or an intermediate. You import the key and cert as a Kubernetes Secret. Then define a ClusterIssuer of type CA that references that secret. cert-manager will use that to sign all Certificate requests 35 36 . This gives you full control and no external dependencies. Ensure the CA's key is kept secure (consider KMS or HashiCorp Vault to store it, or at least restrict secret access severely). - Vault Issuer: If you already have HashiCorp Vault running, you can use Vault's PKI engine as the CA. cert-manager has a Vault issuer type. Vault can then manage the root and intermediate, provide audit logs, and even handle revocation (Vault can revoke certificates on demand). The downside is dependency on Vault availability for issuance and the complexity of maintaining Vault policies. - ACME/ Internal ACME (Not Let's Encrypt): You could run an ACME server internally (e.g. Smallstep's step-ca which supports ACME protocol, or even Boulder if ambitious). cert-manager's ACME ClusterIssuer could then get certs from that. This is only worth it if you prefer ACME's automation but want it offline. In practice, using a CA issuer or Vault is simpler for internal. - Kubernetes CSR API: In theory, Kubernetes has a CertificateSigningRequest API that clusters can sign (often backed by the Kubernetes root CA). We likely do not want to use the Kubernetes CA for service mTLS (it's not SPIFFE-aware and usually for kubelet<->API server). So this is not recommended for our use-case.

Why not Let's Encrypt for internal mTLS? Let's Encrypt (LE) is a fantastic public ACME CA for publicly accessible sites, but it's inappropriate for internal service identities: - Domain Validation Requirements: LE only issues certificates for DNS names or public IPs that you can prove ownership/control of. Your internal services might not even have unique DNS names resolvable publicly (and you wouldn't want to expose internal DNS just to get certificates). On-premise environments or closed networks cannot use HTTP-01 or DNS-01 easily without punching holes to the public internet. - Rate Limits: Let's Encrypt severely limits certificate issuance per registered domain (e.g. 50 certificates per week per domain maximum) $^{37}$ . Microservice architectures with dozens of services, each perhaps needing frequent rotation (say every few hours or daily) will quickly hit these limits $^{38}$ . For example, issuing a couple of hundred certificates in a day is impossible with LE's limits $^{38}$ . LE is designed for human-facing services (90-day certificates, low churn), not high-churn internal credentials. - Operational Fragility: Relying on an external CA means your internal security is now coupled to internet connectivity and a third-party service's uptime. If Let's Encrypt is unreachable (outage or your data center has no internet), your internal certificate renewals fail. For a public website, failure to renew just affects that site; in a microservice system, failure to renew could bring down inter-service auth everywhere once certificates expire. - Trust & Security: Using a public CA for internal trust means that your services would trust a public root (the ISRG Root X1 and others that browsers trust). That

implies any certificate valid under those roots would be accepted unless you implement additional checks. You don't want your internal mTLS to trust every certificate that any browser trusts - that would include a huge number of CAs. Ideally, internal mTLS should have a dedicated trust root that only your org can issue from. (You can restrict acceptance to only specific names, but that's an extra check prone to misconfiguration. Simpler is not even including public CAs in the trust store for internal TLS.) - Identity Semantics: Let's Encrypt issues certificates for domain names like serviceA.example.com. But a SPIFFE identity is not inherently a DNS name - it's a logical identifier. You could shoehorn SPIFFE IDs into DNS names to use LE (e.g. encode spiffe://prod.acme.com/ns/foo/sa/bar as a DNS SAN like bar.sa foo.ns.prod.acme/internal and have LE sign that), but this is very clunky and leaks your internal topology into public DNS. It's also not truly verifying workload identity - it's just verifying domain ownership. In a zero-trust model, domain ownership is too coarse-grained; we want workload attestation which LE cannot do.

Role of Let's Encrypt (or ACME) in this architecture: Limit ACME usage to public-facing ingress. For instance, BRRTRouter as an API gateway or any user-facing web service can absolutely use Let's Encrypt for its TLS certificate terminating external HTTPS. That's where ACME shines - it offloads public PKI management. But for service-to-service, prefer the internal CA. We will thus use LE at the "edge" (ingress) and use our own CA for the "interior" (east-west traffic).

(We will provide concrete guidance in section 4 on exactly how to integrate Let's Encrypt for ingress only.)

# Trust Bundle Distribution and Rotation

A trust bundle is the collection of CA certificates that services use to verify peer certificates 39 18. In our case, the trust bundle will include the internal root CA (and possibly intermediate CA if one is used). Managing this bundle is critical: all services must have the up-to-date bundle, and if we rotate the CA, that bundle must update everywhere atomically.

Options for distributing trust bundles in Kubernetes: - cert-manager's trust-manager: The trust-manager component can watch sources (secrets, configmaps, or references to issuers) and aggregate CA certificates into a Bundle CR, then distribute that bundle to configured locations (like update ConfigMaps or inject into Secrets) 40 41 . For example, we could have a Bundle CR that pulls from the secret containing our root CA and writes to a ConfigMap internal-trust-bundle in each namespace. Pods could mount that ConfigMap or the app could be configured to load it from disk. Trust-manager automates propagating changes (e.g. if you update the CA secret, the ConfigMap updates). - Manual ConfigMap: Simpler, but manual. You create a ConfigMap with the PEM of the CA(s). Every service is configured (via Volume or init container) to use that CA file. If rotation is needed, you'd update the ConfigMap (possibly via a helm release or CI pipeline) and then ensure pods reload it (rolling restart or signaling the process to reload). - SPIRE Agent (for trust): If using SPIRE, the agents distribute the trust bundle automatically to workloads (the bundle comes alongside SVID through the Workload API 42 18 ). If BRRTRouter is modified to retrieve certs via the Workload API, it can also fetch the bundle from SPIRE directly. However, integration of the Rust service with SPIRE's Workload API would require using something like the SPIFFE Rust Workload API client

library (if available) or calling the UNIX socket gRPC. This is doable but adds dependency. An alternative is to run an init container that writes the bundle from SPIRE to a file for the main container.

- In-code bundling: Not recommended, but mentionable: hard-coding the CA certificate in the application. This is secure (immutable) but inflexible (rotating CA requires redeploying code). We prefer using config/secret.

Rotation of trust anchor: Rotating the CA itself (trust domain root) is a special case. Best practice is phase overlap: 1. Deploy a new CA alongside the old (either as new intermediate signed by old root, or if root changes, have services trust both old and new for a time). 2. Issue new certs with the new CA. 3. Once all clients have the new CA in their trust store and all new certs circulated, retire the old CA.

Trust-manager's Bundle can hold multiple certificates, so you can include both old and new during transition 43. BRRTRouter and others, if they simply trust whatever is in the bundle ConfigMap, will then trust both until you remove the old. We should ensure the code doesn't restrict to a single CA certificate pin (unless we update it at rotation time).

The trust bundle must be pinned to our internal root(s) – e.g. in Rustls, we'll create a RootCertStore and add our CA cert(s) to it, not use the system roots. This prevents any external certs from being trusted by mistake. When using an internal CA Issuer, cert-manager's rainjector can automatically inject the CA into secrets of issued certificates (the ca.crt field). But since we do mutual auth, we likely want a single source-of-truth bundle rather than rely on each secret's ca.crt.

# Certificate Consumption by Services (Filesystem vs SDS)

How will BRTRouter and other services get and use their certificates?

If using SPIRE: The service should ideally integrate with SPIRE's Workload API (over a Unix socket) which provides the SVID and key in memory. There is no file I/O; the agent keeps the key in memory and hands it over (protected by Unix domain socket permissions). SPIRE even has an API to push updated certs. However, given BRRTRouter is written in Rust and likely doesn't yet have a workload API client integrated, a simpler interim approach is to run the SPIRE agent with the SPIFFE CSI Driver (which we discussed) to drop the cert and key into a file, similar to the cert-manager CSI approach. SPIRE has its own CSI driver as well (spiffe-csi). Using that, each pod would get a volume like /spiffe-credits with cert.pem, key.pem, and bundle.pem. The service can then read from there. This overlaps with the cert-manager CSI concept, but one would use either SPIRE or cert-manager CSI, not both for the same thing.

If using cert-manager (Option 2A manual): The cert will be output to a Secret (e.g. service-a-tls secret contains tls.crt, tls.key, and ca.crt). We can mount that secret as a volume in the pod. BRRTRouter would then read the files (tls.crt & tls.key) on startup to configure its server TLS. For client TLS (when BRRTRouter calls another service), it also needs its key and cert (to present) - it can use the same cert/key (since each service has one identity for both client and server). So effectively, mount the secret and point BRRTRouter config to those files. On rotation, Kubernetes will update the Secret volume mount (it's eventually consistent - by default there might be some delay). The app would need to detect that file change and reload. A robust plan: run BRRTRouter with a small sidecar or script that triggers a reload signal when the secret is updated (or use an emptyDir + init container to copy, but that complicates rotation).

If using CSI driver (SPIFFE or cert-manager): The certificate and key are in a tmpfs volume and get automatically updated by the driver. The application can either: - Continuously watch the directory for changes (the CSI driver typically replaces the files atomically). - Or periodically attempt to re-read the cert (like before each new TLS handshake if acting as a client). - Or (in case of a server), use a rustls ClientCertVerifier that can call a closure to verify each incoming client cert against the current trust bundle, and possibly even be aware of new bundle content.

Hot Reloading Mechanics: Rustls (used by hyper or tonic) doesn't automatically reload certs. We will implement: - Server-side: BRRTRouter's TLS acceptor holds a ServerConfig object. To reload the cert/ key, one approach is to use ResolvesServerCert trait to dynamically choose the cert at handshake time. We could implement a custom cert resolver that always reads the latest files (though reading from disk on each handshake may be slow - better to watch and cache). Alternatively, BRRTRouter can support a admin signal (SIGHUP) or an endpoint to reload config including TLS. In any case, we must plan for reloading without full downtime. Given our timeframe, an initial implementation might accept a brief connection hiccup for simplicity (e.g. if certificate rotates, log it and require a restart). But since this is a "fintech/ MedTech grade" plan, we want zero downtime rotation. So leveraging notify crate to watch the file and then updating rustls config is ideal.

- Client-side: E.g., if BRRTRouter (as a client) holds an request client with a client certificate, that client likely loads the cert at start. To rotate it, the process might need to create a new TLS client config with the new cert. For long-lived connections (like gRPC channels using tonic), tonic/hyper don't automatically swap client certificates either. A strategy might be to terminate and recreate connections periodically (within the cert lifetime). Or monitor the file and recreate the client. This is complex – an easier route is to use short-lived connections (HTTP calls typically are short-lived anyway) so new connections will use the new cert if the process has reloaded it. We will include a test to ensure that after rotation, any stale connections are dropped or re-established.

Key Usage and SANs (Certificate Profile): We have to ensure the certificates we issue meet the needs of mTLS: - Include Extended Key Usage (EKU) for both Client Authentication and Server Authentication on each certificate $^{25}$ . This enables the cert to be presented by servers and clients. In microservices, a single service often plays both roles (client when calling another service, server when serving requests), so having both EKUs is simplest. - Include a URI SAN with the SPIFFE ID $^{25}$ . There should be exactly one URI SAN, and no other name types (no email, no DNS for internal usage). The CSI driver's approver explicitly enforces that only a single URI SAN is present and nothing else $^{25}$ . We should mirror that practice. This simplifies the verification logic (we don't need to consider multiple SANs or figuring out which SAN is the identity - it will always be the one URI). - Optionally, also include the SPIFFE ID as the certificate Subject CN for human readability. The SPIFFE X.509 spec doesn't require a meaningful subject DN; usually it can be empty or something generic since the URI SAN is the source of truth. We could set the subject CN to the SPIFFE ID string or leave it blank. (No code should rely on CN for identity, as per modern TLS practices; we'll use the URI SAN.) - Set Key Usage: digitalSignature is required (for signing the TLS handshake). We do not need keyEncapsulation if we insist on ECDHE-only cipher suites (which rustls does by default). However, to be safe, enabling keyEncapsulation in case RSA key exchange is used (rustls might not even support RSA key exchange, which is fine). The CSI approver enforces that key usage includes encipherment and signature $^{25}$ , so likely the cert will have both. This is okay. - Short lifetime: We advise something like 1 hour to 24 hours for cert lifetime. The CSI driver uses 1h by default $^{25}$ , which is aggressive (good for security, but ensure renewal mechanisms are solid). If we trust our automation, 1h is great. In case of using Vault, Vault might have a minimum TTL (often 1h is fine for Vault too). If 1h feels too short for initial implementation,

even 24h (1 day) is acceptable, but not much longer. The rotation mechanism should target renewing at least a few minutes before expiry to avoid last-second race conditions.

- Clock Skew & Leeway: When validating JWT SVIDs, BRRTRouter already had a leeway for clock skew.45 46 . For mTLS, clock skew is usually handled by TLS libraries (they accept some small skew in notBefore/notAfter). We should still ensure all nodes are time-synchronized (NTP) because short-lived certs are sensitive to clock differences.

# Failure Behavior and Contingencies

We need to plan for failures in the cert issuance pipeline: - Issuer Outage (cert-manager or SPIRE down): If the CA or issuer is temporarily down, new certificates cannot be issued. With short lifetimes, this is dangerous because if the outage lasts beyond certificate expiration, services will start failing to authenticate. Mitigations: - Use somewhat conservative lifetimes and renewBefore windows. E.g. 1h lifetime, renew at $30\mathrm{m}$ - this gives a $30\mathrm{m}$ buffer. Or 24h lifetime, renew at 12h. - Monitor issuer health. SPIRE Server should be HA; Vault should be HA if used; cert-manager is usually HA (multiple controller replicas). - If an outage occurs, we have an emergency procedure: for example, if SPIRE is down and can't issue, we might temporarily extend the lifetime of existing certs (SPIRE agents cache certs; Vault can issue slightly longer certs if needed; or worst case, manually generate a longer-lived cert for critical services and manually distribute it - not pretty, but a break-glass option). - Renewal Failures: If cert-manager fails to update a cert (bug or misconfiguration), the service's cert might expire. We need monitoring (see Runbook) to catch certs approaching expiry without renewal. In a pinch, an operator could manually trigger a Certificate re-issuance or restart the pod to force CSI driver to recreate request. - Clock skew causing premature expiry: Ensure NTP. If one node's clock is behind, it might reject a perfectly good cert as "not yet valid" or if ahead, see it as expired. Usually not an issue if within seconds, but if there is $>1$ min skew, it's problematic for 1h certs. So enforce clock sync in the cluster.

# 4. Let's Encrypt / ACME: External vs Internal Usage Guidelines

Now to clearly delineate where Let's Encrypt belongs and does not:

Use Let's Encrypt (ACME) for public-facing ingress certificates - e.g., the TLS certificate terminating HTTPS on your BRRTRouter gateway that clients (browsers or apps) connect to. Let's Encrypt excels here: - It's free, automated, and widely trusted by external clients (browsers trust it by default). - BRRTRouter can be set up with HTTP-01 challenge (since it's an HTTP server) or DNS-01 if we can automate DNS updates. For instance, cert-manager can manage a Let's Encrypt Issuer for the api.mydomain.com certificate used by BRRTRouter's public endpoint. - Keep those certs at 90-day lifetime (LE's default) and cert-manager will renew them $\sim 30$ days before expiry. This is fine because external cert rotation has a wide window and is not as latency-sensitive as internal.

Do NOT use Let's Encrypt for internal mTLS between microservices. The reasons, summarized: - Issuance rate limits: As discussed, LE allows 50 cents/week per base domain $^{37}$ . In a microservice environment with potentially dozens of services and Kubernetes pod churn, you could exceed this easily $^{38}$ . Even hitting the 300 orders/3 hours limit $^{47}$ is possible in CI/CD heavy environments. - Domain dependency: LE would require each service to have a DNS name under a domain you control (and one that's public or at least ACME can verify via DNS). Many internal services might not have permanent DNS entries, especially if they are only referred to by Kubernetes service name. Creating and managing all those DNS records (and

ensuring ACME can auth them) is an unnecessary complication. - Security of trust: Using a public CA for internal means trusting public PKI inside. If any one of the hundreds of public CAs (some of which might be less strictly audited than LE) were compromised or mis-issued a cert for your domain, your services might accept it. Keeping internal trust separate avoids that risk. - Reliability: Internal services should not fail because an external service (Let's Encrypt) is rate-limiting or unreachable. We want autonomy.

Edge cases where Let's Encrypt for internal might be seen: There are rare cases - e.g., extremely constrained IoT devices that can only use public PKI or legacy systems that don't support custom roots - but those are not relevant to our Kubernetes microservices scenario. Another edge scenario: if you had a small-scale internal system that for some reason couldn't have its own CA (perhaps to avoid operating any CA at all) you might be tempted to use LE for the few internal connections. Even then, a better approach is to use a free lightweight internal CA (like step-ca).

Final Recommendation: Use Let's Encrypt certificates only on the public gateway interface (and any other public endpoints). For inter-service mTLS, stand up an internal CA via SPIRE or cert-manager. This ensures clear separation of concerns: - Public TLS - terminates at the edge, uses globally trusted CA (LE) because external clients need to trust it $^{48}$ . - Internal TLS - everywhere behind the edge, uses a private trust domain (SPIFFE) and private CA so that we have full control and stronger identity guarantees.

We will now focus on the internal mTLS implementation details, having firmly decided to use an internal CA (not LE) for that.

# 5. BRRTRouter Repository Gap Analysis (SPIFFE/mTLS)

BRRTRouter's codebase already includes some SPIFFE-related functionality, but primarily for JWT SVIDs (token-based auth). A thorough review of the repo reveals several gaps and areas to improve to achieve full SPIFFE mTLS support:

- Lack of X.509 SVID Support (P0): The code explicitly notes that X.509 SVID (mTLS) support is missing $^{49}$ . Currently, SpiffeProvider is designed to validate JWT tokens in HTTP headers (Bearer tokens) $^{50}$ , $^{51}$ . There is no implementation of mTLS handshake handling or certificate parsing for SPIFFE. This is a critical gap - without it, any "SPIFFE support" is incomplete for production zero-trust use. The blog post confirms this is a known issue and a top priority to fix $^{49}$ .   
- No Client Certificate Verification in Router (P0): Searching the code shows no usage of rustls or similar for serving TLS. Likely BRTRouter currently either:   
- Doesn't open TLS ports at all (perhaps it's expected to run behind a TLS terminator).   
- Or if it does, it might allow TLS with a server certificate but doesn't enforce client certs. We saw no config for TLS in config.yaml generation 52 .

This means enabling mTLS will require new code to accept TLS connections and require a client cert. The lack of a TLS listener implementation in code is a gap. We may need to integrate a library like rustls

or use hyper + tokio-rustls if not using may_minihttp for TLS. Possibly may_minihttp has some TLS support via rustls under the hood (to check, but none obvious).

- Trust Bundle & Identity Validation (P0): Even once TLS is integrated, the router must validate the client's SPIFFE ID. A common pitfall is to simply accept any client certificate signed by any CA in a trust store. We must ensure only the intended trust domain's CA is accepted. Right now, since mTLS isn't implemented, there is no validation of certificate URI SAN at all. The design must include:   
- Loading a specific CA bundle (e.g. trust_domain_ca.crt) rather than system certificates.   
- Verifying the certificate's URI SAN is a valid SPIFFE ID and that its trust domain is allowed (likely a configurable whitelist, similar to how SpiffeProvidertrust_domains works for JWT 53 54).   
- Checking that the certificate is not expired and was issued by the trusted CA (rusts will handle basic cert validity and signature checks, but SPIFFE also mandates path component isn't empty etc., which we can parse).   
- No SPIFFE ID extraction from certs (P0): There's currently no code to extract a SPIFFE URI from a certificate. We will need to implement parsing of the X.509 SAN extension. The SPIFFE spec says the URI SAN holds the ID 55. In Rust, we might use the x509-parser crate or webpki via rustls to get SANs. This logic is missing and must be added.   
- No Authorization Layer for SPIFFE IDs (P1): The JWT SpiffeProvider after validation simply returns true/false for auth and presumably attaches the SPIFFE ID somewhere (perhaps in the SecurityRequest). But there is no further granularity - e.g., it doesn't implement rules like "service A's token can or cannot access this route." For mTLS, we likely need an authorization policy to complement authentication. Potential gap: if we enable mTLS and any valid SPIFFE cert from the trust domain is accepted, that might be too permissive. Imagine we want to restrict that only specific services call certain endpoints. The codebase doesn't have a concept of per-service allowlists or ACL. We should design a mechanism (maybe a config section in BRTRouter where you can list allowed client SPIFFE IDs per route or service). This is beyond basic mTLS but critical for multi-tenant clusters or least-privilege enforcement.   
- Certificate Rotation Handling (P1): Since the application will run long-lived, its own certificate and the trust bundle may rotate. Currently no support for reloading config without restart. BRRTRouter does have some hot-reload features (it can reload the OpenAPI spec on changes via notify). We should extend that to watch certificate files. A gap is the absence of a viewer on TLS credits. We can leverage the same notify usage to trigger reloading of the rustls config. Without this, rotated certs would require restarting the service (downtime).   
- Revocation and Expiry Handling (P1): For JWT, there is a RevocationChecker trait (with a no-op implementation) 56. For X.509, there's nothing analogous yet. We likely won't implement full CRL/OCSP, but we should at least log or metric when a client cert is presented that is expired or not yet valid - so ops can detect clock issues or misuse. Possibly, if cert rotation fails and a service's cert expires, how does BRTRouter fail? Ideally it should refuse the connection (which rusts will do automatically if cert is expired). That's fine, but maybe also produce a clear log like "TLS client certificate expired - connection rejected".

- Default TLS Configuration (P1): We need to ensure that when we implement TLS, we use strong defaults:   
- Only TLS 1.2 and 1.3 (rustls doesn't support 1.1 or 1.0, so that's inherent).   
- Strong cipher suites (rusts'safe defaults are good:e.g.ECDHE with AESGCM or CHACHA20).   
- Possibly require client cert always on internal port. If BRRTRouter is also serving public traffic on another port, we might separate ports or have a configuration for "require client auth" on internal listeners and not on public. We must be careful not to accidentally leave a port that doesn't enforce client auth.   
- Validate hostname/ServerName: In mTLS, usually the client verifies the server's identity via the SPIFFE URI (not DNS name). Standard TLS libraries verify DNS names, but here we will instead verify the URI SAN against what we expect. Rustls's ServerCertVerifier can be customized to check the presented cert's SAN. We'll implement a custom verifier that checks the expected SPIFFE ID of the server we're dialing (if the client knows it), or at least that the trust domain matches. For a generic HTTP client, perhaps BRRTRouter will be calling a host name like http://serviceB:8080. We might map that to an expected SPIFFE ID like spiffe://prod.acme.com/ns/xyz/sa/serviceB. A mapping config could be introduced (P2 nice-to-have): a mapping from DNS or service name to expected SPIFFE ID, so that the client can enforce it's talking to the right service, not just any in the trust domain.   
- Metrics & Observability (P2): The current codebase has tracing and metrics (like through OpenTelemetry). There are no specific metrics for security except maybe JWT cache stats. We should add:   
- A metric counter for mTLS handshake failures, with labels for reason (untrusted issuer, expired cert, SPIFFE ID validation failure, etc.). This helps detect if someone is probing with invalid certs or if there's a configuration issue.   
- Possibly a gauge for "days until our own cert expires" that can be scraped, to alert if a service's cert is nearing expiry and not renewed.   
- Logging of client SPIFFE ID on each request (already, after JWT validation, presumably the ID could be logged as part of request context). For mTLS, we should similarly log the client's SPIFFE ID for each inbound request – this is extremely useful for audit trails. We need to pipe the identity from the TLS layer to the request handler (perhaps via a new field in $\boxed{\mathrm{SecurityRequest}}$ or some extension).   
- Configuration Surface (P0/P1): We need new config options for enabling mTLS. Currently, config.yaml has security: api_keys: etc., but nothing about TLS. We should add config keys such as:   
- security.mtls.enabled (bool) - to toggle requiring client certs.   
- security.mtls.trust Bundle_path - file path to the CA bundle to trust.   
- security.mtls.certs_path / key_path - file paths for this service's own cert and private key.   
- security.mtlsAllowed_spiffe_ids - optional list of SPIFFE IDs or prefix patterns that are allowed to connect. E.g. one could list specific IDs or use something like gbbing (spiffe://prod.acme.com/ns/finance/*). If not provided, default allow any from the trust domain. This gives per-service authorization control.

- Perhaps an mtls require-san boolean to require the connecting cert to have a SPIFFE URI SAN (to avoid accidentally trusting other kinds of certs).   
- If BRRTRouter also handles JWT auth on some endpoints and mTLS on others, config might specify which security schemes apply where. But since it's an OpenAPI-driven router, the OpenAPI spec's security requirements might determine that (e.g., an endpoint could declare a security scheme "mtls" – though OpenAPI doesn't natively have mtls scheme except mutualTLS in v3, which is something we can utilize).

None of these exist now, so they must be introduced carefully.

- Code Structure for TLS (P1): The project uses | may_minihttp| for async HTTP. It might need either:   
- TLS support at the library level (maybe via an API to provide an acceptor).   
- Or switching to a stack that supports TLS easily (maybe not trivial; likely better to integrate rusts in the may_minihttp accept loop).

If this is complicated, a stop-gap is running BRTRouter behind an Envoy or NGINX that terminates mTLS and passes through an identity header. But that defers the problem and weakens end-to-end security. Given the question's scope, we assume we want it in Rust code.

# Prioritized Gaps:

- P0 (Must-Fix for Production mTLS):   
- Implement X.509 SVID support: TLS listener with client cert requirement, and verification of SPIFFE trust domain and URI SAN $^{49}$ .   
- Configurable trust anchor loading (no default to system roots).   
- SPIFFE ID extraction and validation (format compliance: must start with spiffe://, have trust domain and path) - similar to is_valid_spiffe_id() used for JWT 57 but for certs.   
- End-to-end mutual TLS handshake integrated into request handling (ensuring that if mTLS is required, a request without a valid cert is rejected early).

- P1 (Should-Fix Soon for Robustness/Security):

- Hot-reload or at least seamless rotation of certificates without full restart.   
- Authorization controls for SPIFFE IDs (so that compromise of one service's credentials doesn't give it carte blanche to call every service).   
- Logging/metrics for TLS events, aiding detect and debug.   
- Integration testing of mTLS (to ensure above works in real scenario).   
- Documentation updates: instruct users how to configure the SPIFFE/mTLS (since this is a new feature).

P2 (Nice-to-Have / Future Enhancements):

- SPIFFE Federation support: allowing multiple trust domains. The blog mentions federation is missing $58$ . Federation would let this service trust identities from another trust domain (e.g. a partner organization) by loading their CA bundle too. Implementing this means supporting multiple trust domains in config and handling mapping of identities. This is advanced; likely future work.

- Automatic CSR generation: possibly have BRRTRouter itself be able to generate a CSR and get a cert (but this starts replicating what SPIRE or cert-manager do – probably not necessary if we rely on external issuance).   
- Support for JWT-SVID authentication on the same endpoints as mTLS (if needed) – bridging the two. E.g., maybe some clients use mTLS, others use JWT – making the system flexible if required.   
- Client-side service discovery integration: e.g. verifying that the SPIFFE ID of a server matches an expected service name. This could tie into service registry: if we know Service B's SPIFFE ID, the client can verify that. This avoids one service with a valid SPIFFE ID calling another service's endpoint unintentionally (though if trust domain is same, it's already authenticated; this is more about authorization, which we already plan to cover via allowlists or policies).

In summary, BRRTRouter currently has a strong foundation for JWT SPIFFE support but lacks the crucial X.509 piece. The gaps identified are critical to close before one can claim "zero-trust" compliance. The above list guides what we implement next.

# 6. Implementation Plan: Hardening BRTRouter for SPIFFE mTLS

To bring BRRTRouter to production-grade mTLS capability, we'll undertake a sequence of implementation steps. This plan covers code changes, configurations, and module additions in detail:

# 6.1 Configuration and Initialization

Extend Configuration Schema: Update config.yaml handling to include mTLS settings. Likely in brrtrouter::config module or similar, add fields such as: - security(mtls.enabled (boolean). - security(mtls CERT_FILE and security(mtls.key_file (strings, paths to PEM files). - security(mtls.trust Bundle_file (string, path to PEM bundle of CA). - security(mtls requirement_client_cert (boolean, default true if enabled - whether to mandate client cert; if we want to allow turning it off for some reason). - security(mtls能够让_spiffe_ids (list of strings for allowed client IDs, optional).

If BRTRouter is used as a library in generated services, these settings would propagate to the generated code's initialization of the server.

Loading Certs and Keys: In startup, if mTLS is enabled, read the certificate and private key from the configured paths. Use Rustls's provided functions to parse PEM:

```rust
use rustls::{Certificate, PrivateKey};
let cert_pem = std::fs::read_cert_path).expect("read cert file");
let key_pem = std::fs::read(key_path).expect("read key file");
// parse PEM to DER
let certs = rustls_pemfile::certs(&mut cert_pem.as.slice());
    .expect("parse certs")
    .into_iter()
    .map(Certificate)
    .collect();
let mut keys = rustls_pemfile::pkcs8_private_keys(&mut key_pem.as.slice())) 
```

.expect("parse private key"); let key $=$ PrivateKeykeys.remove(0));

(We'd handle errors properly, of course.) We assume a single certificate chain and one private key in the files. If the cert is signed by an intermediate, the PEM should include the full chain (except the root).

Loading Trust Anchors: Read the trust Bundle_file (PEM of one or more CA cents). Use rustls_pemfile::cents to get the DER of each, and add to a RootCertStore:

```rust
let mut roots = rustls::RootCertStore::empty();  
for ca in rustls_pemfile::cents(&mut ca_pem.as.slice().unwrap() {  
    roots.add(&Certificate(ca)).unwrap();  
} 
```

This roots will be used for both server-side client cert verification and client-side server cert verification.

# Construct Rustls Configs: - ServerConfig:

```rust
let mut server_config = ServerConfig::builder()
    .with_safe_defaults()
    .with_client_certVerifier(Arc::new(AllowAny AuthenticatedClient::new(roots.clone)))
    .with_single_cert(certs, key)
    .expect("bad certs or key");
server_configverifypeer_name = false; 
```

We use AllowAnyAuthenticationClient which is a Rustls provided ClientCertVerifier that will enforce the client presents a cert chain chaining to one of our roots . We disable automatic DNS name verification ( verifypeer_name=false ) because by default Rustls would try to verify the client's name against some expected DNS - we instead will do SPIFFE URI checking ourselves. Alternatively, we implement a custom ClientCertVerifier to enforce URI SAN, but Rustls doesn't easily give you SAN contents in that verifier without writing more code. A simpler approach: accept any cert from our CA (via AllowAnyAuthenticationClient ), then after handshake, inspect the client cert for SPIFFE ID. If it fails our criteria, we can immediately terminate the connection (e.g., by returning an HTTP 403 and closing). It's slightly late in the process, but still okay. For stricter control, we could implement a custom verifier that calls webpki to extract SAN and check it - doable as a P1 improvement.

- ClientConfig: For BRRTRouter acting as a client (when it calls other services):

```rust
let mut client_config = ClientConfig::builder().with_safe_defaults().with_root_certificates(roots.clone())
. with_single_cert(certs, key) 
```

```javascript
expect("client cert/key"); client_config enable_sni = false; 
```

Here we add our roots so it trusts the internal CA, and we set our own cert for client auth. We disable SNI if we don't have meaningful DNS names (or we could set SNI to some constant like spiffe - since rustls requires something for SNI if TLS1.3, but if enable_sni is false it might skip it). Alternatively, set SNI to the hostname of the HTTP request if calling via DNS that matches internal service DNS, that's fine.

Peer SPIFFE ID verification: By default, rustls client will not verify any DNS name because our internal certs might not have DNS SANs. We must manually verify the server's SPIFFE URI. Rustls allows custom server cert verification by implementing ServerCertVerifier. We can implement one that: - Takes the presented Certificate chain and end-entity, uses webpki to verify it chains to our roots (we may rely on rustls's default for that first). - Then parses the end-entity certificate to extract the URI SAN. Check that: - It exists and is a valid SPIFFE URI (scheme == spiffe, etc.). - The trust domain matches our expected trust domain (which we know, e.g., prod.acme.com). We might configure this trust domain in config (we should, to prevent any rogue trust domain cert being accepted). - Optionally, if the client knows exactly which service ID it's calling, check for an exact match. But in many microservice calls, the client just knows it's calling service B, and if service B uses its own cert, that's the one we got. If we have a mapping of service name -> SPIFFE URI in config, we could enforce it here (P2 feature). - If verification fails, return an error so the TLS handshake fails. In rustls, the ServerCertVerifier returns a Result<ServerCertVerified, TLSError>.

We will implement a simple verifier that enforces trust domain. For instance:

```rust
struct SpiffeServerVerifier {
    trust_domain: String,
    roots: RootCertStore,
} 
impl ServerCertVerifier for SpiffeServerVerifier {
    fn verify_server_cert(&self, endEntity: &Certificate, intermediates: &[Certificate], server_name: &rustls::ServerName, scts: &mut dyn IteratorItemType=&[u8]), ocsp: &OcspResponse) -> Result<ServerCertVerified, TLSError> {
        // 1. Verify the cert chain is valid
        let webpki_roots: Vec<webpki::TrustAnchor<_>> = self.roots.roots_iter().map(|r| webpki::TrustAnchor::try_from_cert_ver(&r.0).unwrap()).collect();
        let end-entity_cert = webpki::EndEntityCert::try_from(end-entity.0.as_ref()).map_err(|_| TLSError::WebPKIError(webpki::Error::BadDER))?;
        end-entity_cert.Verify_is_valid_tls_server_cert( &[&webpki::ECDSA_P256_SHA256, &webpki::ECDSA_P384_SHA384], &webpki_roots, intermediates.iterator().map(|c| c.0.as_ref(), webpki::Time::try_from(SystemTime::now()).unwrap())
        ).map_err(|e| TLSError::WebPKIError(e))?; 
```

```rust
// 2. Parse certificate to extract URI SAN
let cert = x509_parser::parse_x509_certificates(end-entity.
0.as_ref().map_err(|_| TLSError::General("Bad X509".to_string())? .1;
let san_ext = cert.extensions().iter().find(|ext| ext.oid ==OID_X509_EXTSubject_ALT_NAME);
if let Some(ext) = san_ext {
    if let ParsingExtension::SubjectAlternativeName(san) = ext.parsed_extension()
        for name in san.general_names_iter()
            if let_generalName::URI-uri) = name {
                if-uri.starts_with("spiffe:////") {
                    // Check trust domain
                let-uri_td =
            }
            if-uri_tradesWith("spiffe:////").split("\\").next().unwrap_or(""); 
                return Err(TLSError::General.format!( "SPIFFE
trust domain mismatch: {},uri));
            } 
            // If needed, check exact path or allowed list here
            return Ok(ServerCertVerified::assert());
        }
    } 
```

The above pseudo-code uses webpki for chain validation and x509-parser for SAN extraction. We should ensure to include the appropriate crates ( x509-parser or rcgen or ring etc as needed). This verifier would be plugged into ClientConfig:

```txt
client_config.dangerous().set_certificates_verifier(Arc::new(SpiffeServerVerifier{ trust_domain: "prod.acme.com".into(), roots })); 
```

We call .dangerous() because we override the default verification. This is fine since we reimplement proper checks.

This ensures the client not only trusts the root but also confirms the SPIFFE ID belongs to the domain. (If we want multiple trust domains, we'd include their roots and accept if uri matches any of them - that's federation support).

Integrating with BRRTRouter's HTTP server: BRRTRouter uses may_minihttp (which likely listens on a socket and provides an API for reading requests). We need to modify the listening logic to wrap the socket with TLS. We might: - Use rustls::Stream or acceptor to do the handshake. - Possibly switch to tokio-

rustls if may is not asynchronous in the way we need. However, since may is a coroutine runtime, we might have to use rustls in a blocking fashion or ensure the handshake is not blocking others. Perhaps spawn the handshake in the coroutine.

This is a low-level integration detail. The simplest approach: - Use rustls's ServerConnection to process TLS handshake on accept, then wrap the underlying TCP stream in a rustls Stream that implements Read+Write. Then feed that into may_minihttp's HTTP parser as if it was a normal socket. - Alternatively, if may_minihttp doesn't allow easily plugging an alternative stream, we might consider using hyper with rustls for TLS endpoints (but that'd be a bigger refactor).

Given time, we implement it directly: - When accepting a connection, if mtls.enabled, perform:

```rust
let conn = listener.accept(); // raw TCP  
if mtls.enabled {  
    lettlsconn = Arc::new(ServerConnection::new(server_configClone()));  
// perform handshake:  
whiletlsconn.is_handshaking() {  
    let rc =tlsconncomplete_io(&mutconn)?;  
    if rc.0 == 0 && rc.1 == 0{break;} // no progress  
}  
if !tlsconn.is_handshaking() {  
    // handshake done, retrieve client certs  
    let client_certs =tlsconn.peer_certificates();  
    if mtlsrequire_client_cert && client_cert.is_none() {  
        // no client cert provided, we can drop the connection conn.shutdown(); continue;  
    }  
    if let Some Certs = client_certs {  
        // validate SPIFFE ID in certs[0]  
        if !validate_spiffe_cert(&certs[0], allowed_ids, trust_domain) {  
            log::warn!( "Unauthorized SPIFFE ID {"}, /* extract id*/); conn.shutdown(); continue;  
        }  
        // store the SPIFFE ID in connection context for later use  
    }  
    // Now wrap conn in RustlsStream  
    lettls stream = rustls::StreamOwned::new(tlsconn, conn);  
    // Passtls stream to HTTP handling (needs adjustment if may_minihtp  
    expects a specific trait).  
}  
} else {  
    // plaintext, proceed normally  
} 
```

The validate_spiffe_cert function would parse the cert similarly to the verifier above (extract URI SAN, compare to allowed list or at least trust domain). We effectively double-check here what rusts

AllowAnyAuthenticationClient already did for trust chain. This is belts-and-suspenders, plus enforcing our allowlist.

Actually, since `AllowAnyAuthenticationClient` already ensured the cert is signed by our CA, at this point `peer_certificates()` should give at least one cert. We then enforce:

- If `allowed_spiffe_ids` is configured, check that the URI of the cert is in that set (exact match or perhaps prefix match support for wildcard).   
- Else if not configured, we could allow any as long as trust domain matches (to prevent an entirely different domain's cert that happened to be signed by our CA if that scenario is possible - normally our CA wouldn't sign for other trust domains, so this is moot unless we had multiple trust domains on one CA).   
- If fails, drop connection.

If passes, we attach the SPIFFE ID to the request context. Perhaps

`SecurityRequest` struct can carry a field like `spiffe_id: Option<String>`'. When the HTTP request is processed, we populate this from the TLS state. Then handlers or the security provider can use it. In fact, for consistency, we might integrate it with the existing `SecurityProvider` trait:

- We could create an `MtlsSpiffeProvider` that implements `SecurityProvider` (similar to `SpiffeProvider` for JWT). But since mTLS is at connection level, not per HTTP request, this might not fit perfectly into that trait which likely is per request auth. Instead, maybe simpler: if mTLS is enabled for the server, then any incoming connection without a verified SPIFFE ID is rejected before routing; if it has one, we mark the request as authenticated. The OpenAPI security scheme might even define a scheme type "mutualTLS" (OpenAPI 3.1 supports `type: http`, scheme: mutualTLS'). We could map that to our enforcement.

Possibly, BRRTRouter could treat mTLS as one of the security schemes from the OpenAPI spec. If the spec says an operation requires mutualTLS, then at runtime we ensure the connection has a client cert and a valid SPIFFE ID. The design might then integrate into the `SecurityProvider` logic (for example, call a new provider that simply checks if `req.spiffe_id` exists and is in allowed list).

For now, we can keep it simpler: if mTLS is globally enabled, enforce it on all requests. We can refine to per-route if needed using the spec's security requirements.

# 6.2 Client-Side (Outgoing) mTLS Implementation

BRRTRouter-based services may call other services (REST or gRPC). The plan: - HTTP calls (REST): Likely they use request (since it's in dependencies 60). We can enhance the BRRTRouter generated API client (if any exists) or simply provide an example of how to use request with a client cert. request with the rustls- tls feature can accept a ClientConfig. Alternatively, simpler: request has an API ClientBuilder::identity( ) where you provide a PKCS#12 or DER identity. But we have PEM. We might need to combine our cert and key into a PKCS#12 archive for that API, which is not ideal. Another

approach: use request's lower-level rustls support by enabling request's dangerous_tls feature, which allows providing a custom rustls::ClientConfig. However, that feature is literally called dangerous_configuration (allowing custom root without domain verification - but that's exactly our case; we want custom roots). If not wanting to rely on that, we could bypass request and use hyper directly with our rustls config. But since request is in use (maybe for other reasons like JWKS fetch), we might take advantage of it.

Possibly, for initial integration, instruct that when making outbound requests, do something like:

let cert = request::Certificate::from_pem(&std::fs::read("ca.crt").unwrap().unwrap(); let mut client_builderr = request::Client::builder() .add_root_certifcate(cert); // Note: If request doesn't support client identity easily in PEM, one can convert to PKCS12.   
let identity $=$ request::Identity::from_pem(&std::fs::read("clientBundle.pem").unwrap().unwrap(); client_builderr $\equiv$ client_builderr.identity identitiesity);   
let client $\equiv$ client_builderr.build().unwrap();

Where client Bundle. pem contains concatenated cert and private key in PEM. Request's Identity::from_pem expects that. According to their docs, Identity should be a PKCS#12 DER or PEM with both key and cert (and optionally chain). We can ensure to combine the key and cert in one file for this usage. This is a user-space approach; within BRRTRouter, perhaps we don't have a high-level function for making requests (unless the generated code has something).

- gRPC calls (tonic): Tonic uses rustls via tonic::transport::Channel configuration. Example:

```rust
let TLS_config = ClientTLSConfig::new()
    .ca_certificates(Certificate::from_pem(ca_pem))
    .identity(Identity::from_pem(cert_pem, key_pem))
    .domain_name("prod.acme.com"); // or some placeholder, tonic still asks for a domain (it uses it for SNI and verification)
let channel = Channel::from(static("https://service-b:50051"))
    .tls_config(tls_config).unwrap()
    .connect().await?; 
```

Tonic will then do mTLS using rustls. We would have to provide the custom cert verifier if we want to verify SPIFFE URI instead of domain. But if we set domain_name("prod.acme.com"), our custom server verifier earlier would accept any SPIFFE URI in that trust domain. Actually, by default tonic will verify the server's presented cert against the domain_name as a DNS SAN - which our cert doesn't have, so that would fail unless we disable verification. Tonic (via hyper/rustls) can be given our custom verifier by constructing a rustls ClientConfig and passing it in, but that is not trivial through tonic's API (tonic's ClientTlsConfig) allows setting CA and identity but not a custom

```txt
ServerCertVerifier easily, unless we use rustls::ClientConfig directly and use Channel::from(static().connect_withconnector()) with a custom connector). 
```

Given complexity, for now, one approach: Use the spiffe-proxy feature of SPIRE or a workaround - but we want in-app ideally. Perhaps we say: if using tonic, it might be simpler to use DNS names for now with a custom internal CA that signs both URI and DNS SAN. Or we accept that for gRPC, we might need to patch tonic to support SPIFFE better (perhaps a future improvement).

For completeness, we will assume we can manage by using rustls::ClientConfig with custom verifier as above and then do:

```rust
let connector =  
tonic::transport::Endpoint::new(url)?.tls_config(tls_config_from_rustls)?; 
```

```txt
Actually, tonic::transport::Endpoint has a method .tls_config(ClientTlsConfig). ClientTlsConfig in turn can be constructed from a rustls::ClientConfig via ClientTlsConfig::new().rustls_client_config(client_config). This is unstable or recently added possibly (tonic 0.8 or 0.9 might have it). We can check: yes, Tonic's ClientTlsConfig has rustls_client_config(config: rustls::ClientConfig) which consumes the config. So we can indeed plug our custom verifier. 
```

Thus, plan for outgoing calls: - Provide utility or instructions for constructing a request or tonic client that uses the service's own cert and the trust bundle to perform mTLS and enforce trust domain. This might be more of documentation unless BRRTRouter code itself is generating some client stubs.

BRRTRouter could include a small helper like:

```rust
pub fn build_spiffe_client() -> reqwest::Client { ... } 
```

for ease, using environment or config to find the cert files etc. But that might not be in scope for now - we can leave it as guidance for developers.

# 6.3 Integrating SPIFFE ID with Request Handling and Authorization

Passing Identity to Handlers: Once TLS handshake is done and we extracted the client's SPIFFE ID, we should attach it to the request context so that: - It can be logged (tracing can include it). - It can be used in application-level authorization logic (if any, e.g. maybe the business logic might check "if caller is X, allow action"). - OpenAPI security handling: If the OpenAPI spec declares a security requirement (say an OAuth2 or API key) and we have mTLS, do we treat mTLS as fulfilling a requirement? Possibly, if we model mTLS as a security scheme in the spec.

```txt
BRRTRouter likely has a SecurityRequest object (we saw references) that is passed to SecurityProvider.valid() 61. For JWT, SecurityRequest probably holds the Authorization header etc. For mTLS, there is no header; we need SecurityRequest to know about connection info. If 
```

not already present, we might extend it to have a field like peer_spiffe_id: 0ption<String>. We fill this when the connection is established.

Then we create a new implementation of SecurityProvider (or extend SpiffeProvider) to validate based on mTLS: - If mTLS is enabled and required for an endpoint, then validation could be as simple as checking that req. peer_spiffe_id is Some and matches any allowed scope (scopes might not apply here, scopes are more for OAuth). Possibly, we treat possession of a valid SPIFFE ID as already authenticated (the heavy lifting is done in TLS handshake). So the validate() could just return true if peer_spiffe_id exists (and maybe check trust domain if not already done). The heavy checks (like allowed IDs) we might have already done at connection time, but doing them here too is an option.

Alternatively, we might bypass SecurityProvider for mTLS and handle auth at connection layer entirely. But integrating with OpenAPI expectations might be cleaner. OpenAPI 3.0 has type: http, scheme: bearer etc. It has also type: http, scheme: mutualTLS for mutual TLS security (OpenAPI 3.1 introduced a mutualTLS scheme). We can have our codegen mark operations that require mutualTLS, and at runtime, the router enforces that the request came in over an mTLS connection with a verified cert. This could map to a special SecurityProvider or just a condition.

Authorization (Allow list by service): We likely want a configuration to limit which SPIFFE IDs can access which service. This is beyond OpenAPI (which typically doesn't specify specific client identities allowed). It might be more of a runtime policy. Possibly, we integrate with an external policy engine (OPA/OPAL) in future, but for now a simple allowlist in config is good. If allowed_spiffe_ids is set, our connection handling already uses it to reject unauthorized IDs. That effectively enforces that only those IDs can connect at all. That might be coarse, but sufficient if each service knows exactly who should call it. If more granular (per endpoint) rules are needed, we'd need an internal mapping from endpoint -> allowed IDs. That could be configured via extension fields in the OpenAPI (like a custom x-allowed-ids). This is beyond current scope, so we'll do simple service-level allow.

Preventing fallback to insecure: Ensure that if mTLS is required, the server does not accidentally accept the same request over an insecure channel. This likely means if mtls.enabled=true, we might want to disable the HTTP listener on the same port without TLS. Or if both are offered (some systems do that for backward-compatibility), at least mark the endpoints so they know if they're secure or not. Better to just have a single listener that requires TLS. Or separate ports (8443 for TLS, 8080 for plain) and not expose 8080 in production. We should default to no plaintext when mTLS is on.

# 6.4 Code Module Additions

New modules or crates: - Add rustls (and rustls-.pem file) to dependencies. Possibly update Cargo.toml to include tokio-rustls if needed for async. If staying with may, not needed. - Add x509-parser = "0.9" or similar to parse SAN easily, or use rcgen which can parse certs (rcgen is more for generating). - Use webpki which comes with rustls for chain verification if custom. - Feature-flag dependencies if needed (maybe behind a "mtls" feature to keep it optional for those not using it? Could do, but if we consider mTLS a core feature, include it by default). - Potentially use notify (already indeps) to watch files: - We can spawn a background thread (or coroutine) to watch the cert and key files for changes. On change, re-read them and update the rustls config. Rustls ServerConfig doesn't allow swapping certs easily on the fly, but if we use a ResolvesServerCert trait impl that holds the current cert in an

$\boxed{\mathrm{Arc<RwLock<...>>}}$ , we could update that. Alternatively, simpler: trigger a graceful restart of the accept loop. But that's not ideal. We can implement a custom ResolvesServerCert that always returns the latest cert from memory. For initial cut, a restart might be acceptable (with rolling restarts, you won't drop service). - Logging: ensure tracing:::warn! or debug! are used as appropriate when certificate validation fails or when reloading is done.

# 6.5 Implementation Steps Summary

1. Config struct changes - add mTLS fields; parse from YAML (update serde derives).   
2. Load keys/certs at startup; construct rustls config objects.   
3. Listener modification - wrap accept to do TLS handshake if enabled.   
4. Identity extraction - after handshake, parse and validate SPIFFE URI; store it in request context.   
5. Connection handling - if no valid identity (and required), drop connection (maybe return 401-like error if we can send an HTTP response without identity? Possibly not, better to just close).   
6. Link to router logic - ensure that only authorized IDs are allowed (either at connection or via SecurityProvider).   
7. Outgoing client support - provide guidance or helpers for making mTLS requests (not strictly needed in core, but good to document or have as util).   
8. Hot Reload - set up file watchers on cert and key (and maybe CA bundle if we foresee CA rotation). On change, log it and reload:   
9. Reload own cert/key: create new rustls ServerConfig and update the acceptor (this might not affect existing connections, which is okay - they'll continue with old cert until done).   
10. Reload trust anchors: update RootCertStore for new connections (existing connections aren't affected until they re-connect).   
11. Possibly, for long-lived connections (like gRPC streams), if trust bundle changes (e.g. CA rotation), those connections will continue under old trust until reconnected. That's normal unless we implement dynamic revalidation, which is not typical for TLS.   
12. Testing hooks - include a way to simulate or force reload (for test, maybe accept a signal or admin endpoint to reload to avoid waiting for file change).

By following these steps, we will embed SPIFFE mTLS deeply into BRRTRouter. The end result is that if mTLS is enabled, every inbound request is cryptographically authenticated with a SPIFFE ID before it even hits the application logic.

We should also ensure to document how to configure this (which likely goes in README or docs).

# 7. Testing Strategy

Implementing mTLS and SPIFFE identity requires thorough testing at multiple levels:

# 7.1 Unit Tests (Library-level)

Write unit tests for new functions: - SPIFFE ID parsing from X.509: Create a dummy certificate (we can generate one on the fly with a known URI SAN using rcgen crate) and test that our parsing function extracts the correct SPIFFE ID string and validates trust domain correctly. Also test edge cases: no URI SAN,

multiple SANs, malformed URI (should be rejected). - Allowed SPIFFE ID logic: If allowed_spiffe_ids is set, test that a cert with an ID not in the list gets rejected. Test pattern matching if we support wildcards (e.g. allowed prefix spiffe://prod.acme.com/ns/allowed/, then an ID under that prefix passes, others fail). - Certificate Verification logic: We can use a self-signed CA in tests. For example, generate a CA key, sign a cert for a "client" SPIFFE ID and a "server" SPIFFE ID. Use our ClientCertVerifier or ServerCertVerifier functions directly to ensure they accept valid combos and reject bad ones: - A cert signed by unknown CA -> should be rejected by rustls inherently. - A cert with wrong trust domain in URI -> our custom check should reject 62 63 . - Missing SPIFFE URI -> reject. - Expired cert -> ensure rustls rejects (we can manipulate webpki::Time in tests by not using real SystemTime or set a short validity). - Future-dated cert (not yet valid) -> should reject (webpki handles that via time).

- Hot Reload file viewer: Possibly simulate by writing a temporary file and triggering notify events. Or abstract the viewer logic so we can call a function to simulate "cert updated" and see that the internal config updated. We might need to expose or log something we can assert on (like a metric or a variable indicating current certificate serial).   
- Revocation (if any logic): Not doing CRL/OCSP, so nothing unit testable there beyond expiration.

# 7.2 Integration Tests

Set up integration tests that run a BRRTRouter instance (perhaps in-memory or as a subprocess) with mTLS and ensure it communicates correctly: - Self-contained mTLS handshake test: Use a simple route (like a health check endpoint) that requires mTLS. Write a test client that attempts to connect: - Case 1: With a valid client cert (issued by test CA, with correct SPIFFE ID). Expect HTTP 200 (or some response). - Case 2: Without a client cert. Expect connection drop or no response (if our server simply closes). - Case 3: With an invalid cert (signed by different CA or wrong trust domain). Expect handshake failure (could manifest as connection reset or perhaps our server writes a 403 then closes).

We might need to use a lower-level socket to observe handshake result if it fails at TLS layer (since no HTTP will happen). Possibly a try/except around the client connection attempt to see it was refused.

We can leverage request for testing by configuring its client identity appropriately: - Use request without client cert to simulate unauthorized: it should error (maybe "connection closed"). - Use request with proper identity (we can load our test key/cert into request::Identity). It should succeed and get a response.

BRRTRouter being asynchronous, we can spawn it in a thread. Alternatively, compile a small test binary with BRRTRouter config and run it. But since we have the library, we could instantiate brtrouter::router directly. However, it might not have an easily callable interface, and may rely on CLI args. Possibly easier: generate an OpenAPI spec for a trivial service, use the codegen to produce a server, then run it. That's a bit heavy for tests. As a compromise, BRRTRouter might have integration tests already (there is tests/spiffe/tests.rs which did use Docker to simulate JWKS etc.). We can follow that pattern.

Focus scenarios: - Expired Certificate Handling: Create a certificate that is already expired (set validity window in past or short). Try connecting with it - rustls client likely aborts handshake. On server side, see that it was rejected. Possibly inspect logs for "expired" warning. We can't easily capture logs in integration test unless we hook a custom logger (or check if handshake returns an error code). - Rotation scenario: This is a bit complex to test in an automated way, but we can attempt: 1. Start server with initial cert. 2. Perform a request - should succeed. 3. Replace the server's certificate and key files with a new cert (maybe

signed by same CA or a new CA that we also update trust). 4. Signal the server or wait for viewer to pick it up. 5. Perform another request – should succeed with new cert (the client might need to trust the new CA if we changed it).

If we keep same CA but just a new leaf cert, the client's trust doesn't need change, only server's own cert updated. The client might not notice - but if the client reused a connection from before, it might still be using old cert (though typically server cert is fixed per connection; new connections get new cert). We ensure to create a new client connection after rotation. Then we could verify, e.g., by checking the server's certificate presented is the new one (the client can inspect the server cert via something like rustls:::Session in a custom client or using OpenSSL s_client via command line in a pinch).

This is quite involved. If time, at least manual test in a dev environment.

- Multiple trust domains (if implemented): Not for initial; skip.   
- Rogue certificate scenario: Simulate an attacker service that somehow got a cert signed by our CA but with a SPIFFE ID not allowed.   
- If we have allowed list, generate a cert for an ID not in list. Ensure connection is rejected.   
- If no allowed list, then as long as CA is same, currently we would accept it (because how would our server know it's "rogue" if trust domain same? Only if we implement finer authorization or if perhaps the path in SPIFFE ID can convey service type and we code certain trust domain subdivisions. For now, one trust domain, any valid cert is accepted if not explicitly allowed or denied. We could simulate by marking an ID as not allowed in config and test that.

Essentially test that allowed_spiffe_ids config works as expected.

- Downgrade attempt (plaintext): If someone tries to bypass mTLS by calling an HTTP endpoint on the service's port without TLS. This depends how we set up the server:   
- If we use one port that expects TLS handshake, a non-TLS client connecting and sending HTTP plaintext will either be gibberish to rustls (handshake fails) – effectively, connection closes. The client would see an error (maybe TLS handshake failure if it was expecting some, or just closed if it wasn't).   
- This is good (they can't speak plaintext on the TLS port).   
- If we inadvertently left another port open for HTTP, that's misconfig. We should test that when mt1s.enabled, perhaps we don't open the plaintext listener (or if we do, it's on a different port maybe for metrics but not for API).   
- We can attempt to connect to the service's port using plain TCP and send an HTTP GET without TLS from a dummy client - expect no valid response or connection drop.

In integration, we can do a TcpStream::connect and write b"GET /healthz HTTP/ 1.1\r\nHost: ...” to it directly to see if we get anything. We likely won't, as rustls server would interpret that as handshake bytes, fail, and close.

# 7.3 End-to-End in Kubernetes (E2E)

This might be more of a deployment test rather than code test, but ideally: - Deploy a minimal BRRTRouter-powered service in a kind cluster with cert-manager and SPIRE to verify the whole flow with real certificates.

This would be part of a QA or pre-release checklist rather than automated CI (unless we integrate KinD in CI). - Still, we can write a script or documentation for how to do this: - Setup cert-manager Issuer & CSI driver in a test cluster. - Deploy the service with annotation for CSI volume or mount the secret from cert-manager. - Curl the service from another pod using appropriate cert and see that it requires mTLS.

Given complexity, we may not fully automate E2E in CI, but we will have a manual test plan in the runbook.

Test of failure modes specifically: - Expired cert: Already covered - simulate by using an expired client cert. Expect handshake failure. Also simulate server cert expiration if possible by setting a short TTL and waiting - but in automated test that's tough unless we manipulate system time or can dynamically replace server cert with one that is expired (which we can do by generating one). - CA Trust bundle mismatch: Simulate by having the client trust a different CA than the server uses. E.g., client has not updated trust bundle after CA rotation. Then client's handshake fails (unknown issuer). On server side, we might see no client cert (if client never completed handshake). So the server might not log anything particular except maybe rustls handshake error. We could assert that a client error contains "unknown issuer". - Simultaneous trust anchor changes: For federation or rotation, maybe out of scope for initial tests.

All in all, tests will ensure: - The mTLS handshake occurs as expected. - Only authorized connections get through. - Identities are correctly extracted and passed to application (maybe we add an echo endpoint that returns your SPIFFE ID, to test that the server actually captured it). - No regressions to existing features: ensure that if mTLS is off, the server still works with JWT auth, etc. Run existing tests to confirm JWT paths untouched.

# 8. Operational Runbook (PCI-Grade Considerations)

Even with perfect implementation, operating a SPIFFE/mTLS system requires careful processes. Here we outline runbooks for key operational tasks and failure scenarios:

# 8.1 Certificate and Key Rotation Playbook

Workload Certificate Rotation (Normal): Certificates for workloads (services) will be short-lived and autotrotated. Normally, no human intervention is needed - SPIRE or cert-manager handles it. However, operators should: - Monitor certificate age: Have dashboards or scripts that list all service certificates and their expiration. For example, if using cert-manager, you can run kubectl get certificates to see statuses or use metrics from cert-manager (certmanager_certicate_not_after metric). - Ensure that renewal happens (cert-manager sets a condition, SPIRE rotates automatically). - If a certificate is nearing expiry and not yet renewed (e.g., <10 minutes left), treat it as an incident: possibly restart the pod to trigger re-issue, or investigate cert-manager logs if something stuck.

CA (Trust Anchor) Rotation: This is rarer (maybe yearly or in compromise). Plan: - Preparation: Decide if using an intermediate or rotating root directly. Often, you introduce a new intermediate signed by the current root (to avoid touching root in short-term). - If rotating root CA: 1. Generate new CA key and cert (keeping secure). 2. Distribute the new CA in addition to the old CA: e.g., update the trust bundle ConfigMap to include both old and new. 3. Update Issuer to use the new CA for signing new workload certificates. (In SPIRE, you'd configure an UpstreamAuthority or just cut over after a certain time). 4. Trigger rotation of all workload certificates (for cert-manager, you might label or force reissue; in SPIRE, just waiting until TTL or

bump TTL shorter to accelerate). 5. Monitor that new certs (with new CA) are being served and accepted (because clients trust both). 6. Once all workloads have switched (give some margin, say double the max TTL to be sure), remove the old CA from the bundle. 7. At this point, any straggler using old CA will fail - ensure none remain. Then revoke old CA and archive it.

- If using intermediate:   
- Perhaps easier: keep root the same, just rotate intermediate that signs workload caps. In that case, trust bundle remains the root (unchanged); only the intermediate key changes which is transparent to workloads as long as root is trusted. This is simpler: just install a new intermediate on CA Issuer or SPIRE (via UpstreamAuthority signing a new intermediate), and revoke the old intermediate.

Workloads get new chain. Because root didn't change, you don't need to change trust in clients. This approach is recommended if you can afford to keep a stable root for a long time (which you protect offline).

Key Compromise Response (Workload): If a specific service's key/cert is suspected stolen: - Revoke its credentials: In SPIRE, you can delete the registration entry so it no longer gets that SVID (and bounce the workload). In cert-manager, if it's a one-off secret, you can delete the Certificate and secret - the CSI driver will request a new one (with new key). Also consider adding the cert to a CRL (if using Vault, vault pki revoke does that, and you can distribute CRL via trust-manager Bundle; if using a simple CA Issuer, you might not have CRL capability - in that case, short TTL is our mitigation). - Ideally, notify the service owner and push a new deployment of that service to rotate keys (if not auto). In a SPIRE environment, you could tell SPIRE to ban that specific SVID by serial number (SPIRE doesn't have a built-in CRL, but since TTL is short, it's okay). - If we suspect attacker might use the stolen cert to impersonate that service to others, consider also updating authorization rules to block that service ID temporarily from sensitive services until it's rotated. - If multiple keys compromised or root cause unknown, consider rotating the CA entirely (worst case).

Key Compromise (CA key): This is a disaster scenario – if the root CA is compromised, the entire trust domain is at risk since attacker could sign anything. Immediate steps: - Treat it as a full security incident (incident response team involvement). - Rotate the root CA immediately: effectively re-establish trust domain with a new root. This means all workloads must get new certificates under the new root and distrusting the old root. This is like performing the trust anchor rotation steps under duress, likely with downtime. - In practice, one might shut down inter-service communication (isolate services) if possible while injecting the new root, to prevent the attacker from using the compromised one to pivot. - After rotation, audit and possibly redeploy all workloads with fresh credentials. - Publish the old root's certificate to a revocation list that all services will henceforth reject (since we'll remove it from trust bundle, any certificate from old root will fail anyway). - Post-mortem to identify how it was compromised and ensure it doesn't happen again.

# 8.2 Incident Response for mTLS Failures

Consider scenarios where mTLS might break and how to respond: - All mTLS handshakes failing (wide outage): Possibly the CA expired or was removed incorrectly, or trust bundles not propagated. Action: quickly assess if the trust bundle ConfigMap was updated or Issuer is down. If CA expired, try to re-add a valid CA (maybe the new one) to bundles and restart pods with new certs. If Issuer down, bring it up or in extreme case, flip to a backup CA (maybe have a second CA in config that can issue emergency credentials). - Single service failing to communicate: e.g., Service A can't call B due to auth error. Possibly A's cert expired (so B rejects it) or B's trust store is missing A's CA. Check logs: on A side, if it sees server verification

error - likely B's cert/trust issue; on B side, if it logs unauthorized client - likely A's issue. Renew certificate for A if needed. If an allowlist is blocking it (maybe config was wrong), fix config and reload B. - High handshake failure count: If metrics show a spike in failed TLS handshakes from a particular source, possible causes: - Someone is port-scanning or sending non-TLS traffic (could be an attacker or a misconfigured client). Investigate source IP; if external, it may be an attack (shouldn't reach internal services ideally; if internal, maybe a misdeployed pod without proper cert). - A service's certificate might have just expired (we'd see it failing to authenticate to others). Check if one service's metric is failing everywhere - that service likely lost credentials. - Wrong trust domain usage: maybe a service from a different environment tried to connect (if federation not configured, it'll fail). That could be benign or misconfig.

Procedure: When an alert triggers (e.g., "mTLS handshake failures > X/min"), have runbook steps: 1. Identify which service(s) are failing and when it started (correlate with any deploys or cert rotations at that time). 2. If it's an expiration issue, rotate the credentials (if automatic failed, do manual). 3. If it's a trust mismatch (e.g., CA rotated but some didn't get update), push the correct trust bundle quickly (maybe restart trust-manager or reapply Bundle). 4. If suspect malicious activity (unknown certificates), you might grab the peer certificate that was presented (if our logs captured it or if we can run in debug mode to capture it). Forensic analysis on that cert (issuer, SPIFFE ID claimed, etc.) could identify if it was a legitimate cert from our CA or something else. 5. Based on that, either tighten allowlists (if someone from an unexpected service is calling where they shouldn't) or update security policies.

# 8.3 Revocation and Emergency Response

As mentioned, we rely on short TTL rather than revocation lists, but some emergency steps: - If needed, we can compile a CRL (Certificate Revocation List) of serial numbers to revoke. If using Vault, simply revoking an issuance puts it on a CRL distribution point. Our services would need to check CRLs – rustls doesn't do that by default. We did not plan CRL checking in our implementation (since short TTL was the strategy). If this becomes a requirement (PCI might like revocation methods), an improvement is to distribute CRLs via trust-manager (it can include CRLs in bundles perhaps) and perform CRL check in custom verifier. This is complex and not realtime (CRLs usually updated periodically). Alternatively OCSP stapling could be considered – but our environment is closed, OSCP is overkill for internal.

If a cert must be immediately invalidated, the pragmatic approach in our design is: - Remove its trust by removing the CA if possible (not ideal, as it affects all). - Or push an update to all servers to add that cert's SPIFFE ID to a denomlist in config and hot-reload. We could maintain an in-memory denomlist of banned SPIFFE IDs (that we can update via config map or management API). Then even if the cert is technically valid, the server would reject it if the SPIFFE ID is on the ban list. This is workable for targeted revocation. We should document that procedure: - E.g., edit a ConfigMap brtrouter-security.yaml to add under security.mtls revoked_ids: ["spiffe://prod.acme.com/ns/foo/sa/bad"]. Trigger the router to reload config (SIGHUP or via its viewer if we implement one for config). - BRTRouter then in its allow logic, denies any connection with that ID.

This provides an out-of-band revocation mechanism application-level. (Marking as P2 feature possibly, but operators can also just scale down the service with that ID if compromised).

# 8.4 Monitoring and Alerting

To run this in production, set up monitoring for: - Certificate Expiry: Alert if any service cert is $< X$ hours from expiry with no new cert in place. If using cert-manager, use its Prometheus metrics or write a small cronjob that checks kubectl get certs -o json for statuses. -SPIRE Health: If using SPIRE, monitor SPIRE Server and Agent status (they have telemetry metrics and can send heartbeats). If an agent dies on a node, workloads on that node might fail to rotate. - Handshake failure metrics: As planned, BRRTRouter will expose counters for handshake failures. Create alerts: e.g., " $>5\%$ of connections failing TLS negotiation" or a sudden spike, alert DevOps. - Unauthorized identity attempts: If we log warnings for disallowed SPIFFE ID attempts, aggregate those logs. A single occurrence might be misconfig, multiple could be malicious. For compliance, keep those logs - they are evidence of attempted unauthorized access. - Volume of rotations: Track how often certificates rotate. If something is causing them to rotate too frequently (misconfiguration causing issuance on every pod restart?), it might indicate inefficiency or an attacker repeatedly triggering restarts. Normally, rotation should be regular (e.g. each hour for each pod). - Performance overhead: mTLS adds CPU overhead. Monitor latency and CPU after enabling mTLS. It should be acceptable (rusts is fast, and using ECDSA + AES is usually fine, but at 100k rps maybe noticeable). If overhead is high, consider tuning (e.g. reuse connections, etc.).

# 8.5 Disaster Recovery

Keep backups of CA keys (especially if using a custom root stored in K8s secret - have that secured and backed up offline). If the cluster is lost, you need the CA to restore trust (else all new certs would not match old trust, requiring redeploy of all clients or emergency trust change). - If using Vault, ensure Vault unseal keys and recovery are in place. - Document the process to recover SPIRE server (which contains the trust domain private key) onto a new server if original is lost.

# 8.6 Compliance Considerations

For PCI or similar: - Ensure all TLS settings meet standards (e.g., TLS1.2+ required by PCI DSS). - Document who has access to CA private keys (should be limited, with possibly HSM). - Regularly rotate keys (there might be a requirement to rotate root every X years). - Logging of access: Each mutual TLS connection ideally can be traced to a service identity. We log the SPIFFE ID for every request - this can feed into SIEM for audit (e.g., "serviceA called endpoint /transfer on serviceB at time T with identity spiffe:/../serviceA"). - Penetration testing: attempt to use a cert from another environment or a self-signed - confirm system rejects it. - Ensure that if an attacker compromises one service, they cannot impersonate another - thanks to unique SVIDs and allowlists, they cannot (unless they also compromise the CA or issuance mechanism).

By following this runbook, the team will be prepared to operate the SPIFFE-based mTLS with high confidence, and handle any incidents swiftly.

# 9. Deployment Blueprint and Examples (Recommended Defaults)

To solidify the plan, here is a blueprint of configuration and code snippets for a default deployment using cert-manager + SPIFFE CSI driver (Option 2), which is a likely choice for Kubernetes clusters:

# 9.1 Kubernetes Manifests (cert-manager Issuer, Certificate, Bundle)

First, set up an internal CA and cert-manager issuers:

```yaml
# Internal CA secret (contains our root CA key and cert)  
apiVersion: v1  
kind: Secret  
metadata:  
    name: internal-ca-secret  
    namespace: cert-manager  
type: kubernetes.io/tls  
data:  
    TLS.crt: <base64 of ca.crt>  
    TLS.key: <base64 of ca.key>  
---  
# ClusterIssuer that uses the above CA  
apiVersion: cert-manager.io/v1  
kind: ClusterIssuer  
metadata:  
    name: internal-spiffe-ca  
spec:  
    ca:  
        secreName: internal-ca-secret  
---  
# trust-manager Bundle to distribute CA to all namespaces (optional)  
apiVersion: trust.crt-manager.io/v1alpha1  
kind: Bundle  
metadata:  
    name: internal-trust-bundle  
spec:  
    sources:  
        - secret:  
            name: internal-ca-secret  
            key: TLS.crt  
        target:  
            configMap:  
                name: internal-trust-bundle  
                key: bundle.pem  
# This will create a ConfigMap 'internal-trust-bundle' in the cert-manager "trust" namespace by default containing the CA.  
# We can configure trust-manager to copy it cluster-wide or mount this one where needed. 
```

(In this blueprint, we use a single root CA. In production, you might use an intermediate CA: in that case, internal-ca-secret holds the intermediate cert and key, and trust bundle would include the root and maybe intermediate.)

Now, for a service deployment using the SPIFFE CSI driver to get its cert:

```yaml
# SPIFFE-enabled Deployment for Service A
apiVersion: apps/v1
kind: Deployment
metadata:
    name: service-a
    namespace: backend
spec:
    replicas: 1
    selector:
        matchLabels: { app: service-a }
    template:
        metadata:
            labels: { app: service-a }
    spec:
        serviceAccountName: service-a-sa
        containers:
            - name: service-a
                image: myregistry/service-a:latest
                ports:
                    - containerPort: 8080
                    volumeMounts:
                        - name: spiffe-credits
                            mountPath: "/var/run/secrets/spiffe.io" # CSI will mount certs here
                            readOnly: true
            env:
                - name: BRRTROUTER_MTLS_ENABLED
                    value: "true"
                    - name: BRRTROUTER_MTLS_cert_FILE
                    value: "/var/run/secrets/spiffe.io/svid.pem"
                    - name: BRRTROUTER_MTLS_KEY_FILE
                    value: "/var/run/secrets/spiffe.io/svid.key"
                    - name: BRRTROUTER_MTLS_TRUSTBundleLE
                    value: "/var/run/secrets/spiffe.io/bundle.pem"
volumes:
    - name: spiffe-credits
    CSI:
        driver: spiffe.csi_cert-manager.io
        readOnly: true
        volumeAttributes:
            issuer: "internal-spiffe-ca"
# The CSI driver will use the service account token to request a
cert for spiffe://<trustDomain>/ns/dashboard/sa/service-a-sa 
```

Trust domain is configured in the CSI driver's ConfigMap usually (e.g. default "example.com" or set via driver args).

In the above: - The CSI driver will create files: svid.pem (cert chain), svid.key (private key), and bundle.pem (the CA bundle) in the mount. - We pass those paths to BRRTRouter via environment (assuming we map env to config fields in code). - We set BRRTROUTER_MTLS_ENABLED=true to turn on mTLS mode. (We would parse that in config to security.mtls.enabled). - Ensure the issuer volumeAttribute matches the ClusterIssuer name. The trust domain is typically configured cluster-wide for the CSI driver (if not, it might default to something; in config we might need to set it - but in our driver config in the cluster, we'd specify --trust-domain=prod.acme.com as an arg).

For completeness, an example of configuring the CSI driver (not asked, but to illustrate):

```yaml
Assuming cert-manager-csi-driver is installed via Helm or manifest, ensure it's configured with trust domain:  
# e.g., in the Deployment for spiie:  
containers:  
- name: spiie-driver  
    args:  
        --driver-name=spiffe.csi.cert-manager.io"  
        --trust-domain=prod.acme.com"  
        --issuer-name=internal-spiffe-ca"  
        --issuer-kind=ClusterIssuer"  
        --issuer-group=cert-manager.io" 
```

That would default any request to use that issuer and trust domain if not overridden per volume.

# 9.2 BRRTRouter Service Configuration

If BRRTRouter reads a YAML config, we could mount a ConfigMap or pass via env. Using env as above is fine (the code can translate env to config). If using a config file, it might look like:

```yaml
# config.yaml for service A   
security: mtls: enabled: true cert_file: "/var/run/secrets/spiffe.io/svid.pem" key_file: "/var/run/secrets/spiffe.io/svid.key" trust Bundle_file: "/var/run/secrets/spiffe.io/bundle.pem" require_client_cert: true allowed_spiffe_ids: - "spiffe://prod.acme.com/ns/backend/sa/service-b-sa" - "spiffe://prod.acme.com/ns/frontend/sa/gateway-sa"   
http: 
```

```txt
port: 8443
# ... other settings ... 
```

This config says: - mTLS is on. - Trust only the certs in bundle.pem (which should contain the prod.acme.com CA). - Our service will present svid.pem/key. - Only allow two specific SPIFFE IDs to call this service (perhaps we expect only service B and the API gateway to call service A). This is an example of narrowing access. - The service listens on port 8443 (could also use 443 if running as rootless with CAP_NET_BIND, but 8443 is fine). - If needed, we might also have an option for a separate HTTP port for non-mTLS. In a secure deployment, that would be off. But if we needed one for health checks, we might run a health server on localhost only.

# 9.3 Minimal Rust Code Examples

Server-side using rustls (simplified):

Here's a minimal snippet showing how a Rust server could handle mTLS with rusts and extract SPIFFE ID:

use std::sync::Arc;   
use rustls::{ServerConfig, Certificate, PrivateKey, RootCertStore, ServerConnection, StreamOwned, server::AllowAnyAuthenticationClient};   
use std::net::TcpListener;   
use x509_parser::prelude: $\text{串}$ .   
fn load_certs(path: &str) -> Vec<Certificate> { /* read PEM and parse as above   
\*/ }   
fn load_private_key(path: &str) -> PrivateKey { /* read PEM and parse */ }   
fn load_trust/store(ca_path: &str) -> RootCertStore { /* as above */ }   
fn extract_spiffe_id_from_cert Cert: &Certificate) -> Option<String> { // Parse certificate DER to extract URI SAN let (_, cert) $=$ parse_x509_certifcate(cert.0.as_ref().ok(   )?); for ext in cert.extensions() { if let ParsingExtension::SubjectAlternativeName(san) =   
ext.parsed_extension() { for name in san.general_names_iter(   ) { if let GeneralName::URI.uri) $=$ name { if uris.starts_with("spiffe://") { return Some(uri.to_string(   )); } 1 } 1 } } } } } } } } } } }

```rust
fn main() { // Load our credentials and trust anchors let certs = load_cert("svid.pem"); let key = load_private_key("svid.key"); let roots = load_trust/store("bundle.pem"); let client_auth = AllowAnyAuthenticatedClient::new(roots); let server_config = ServerConfig::builder().with_safe_default() .with_client_certVerifier(Arc::new(client_auth)) .with_single_cert(certs, key) .expect("invalid cert or key"); let listener = TcpListener::bind("0.0.0.0:8443").unwrap(); println("Server listening on 8443..."); for stream in listener.incoming() { let tcp_STREAM = stream.unwrap(); let mut TLSconn = ServerConnection::new(Arc::new(server_configClone()))).unwrap(); if let Err(e) = TLSconn-complete_io(&mut (&tcp_STREAM)) { eprintln("TLS handshake failed: {:?}", e); continue; // reject connection } // Handshake done let client_cert = TLSconn.peer_certificates(); if client_cert.is_none() { eprintln("No client certificate provided, closing "); continue; } let peer_cert = &client_cert.as_ref().unwrap()[0]; let spiffe_id = extract_spiffe_id_from_cert(peer_cert); if spiffe_id.is_none() { eprintln("Client cert missing SPIFFE ID, closing "); continue; } let spiffe_id = spiffe_id.unwrap(); println("Accepted connection from {:}", spiffe_id); // Here we could enforce allowed IDs: // if !ALLOWED_IDS.contains(&spiffe_id) { close connection } // Wrap the TCP stream in rustls stream let mut TLSStream = StreamOwned::new(tlsconn, tcp_STREAM); // Now we can read HTTP request from TLS_STREAM... let mut buf = [0u8; 1024]; let n = TLSStream.read(&mut buf).unwrap(); let request = String::from utf8_lossy(&buf[..n]); println("Received request {:}", request); // ... handle HTTP and write response ... } } 
```

This simplified example uses blocking I/O for clarity. In BRRTRouter, it would integrate with its coroutine runtime, but the essence is: - Accept TLS handshake with client auth. - Verify and extract SPIFFE ID. - Use it for authz (allowed list check). - Proceed to handle request on the TLS stream.

Client-side (Rust) with request:   
```rust
use request;
use std::fs;
fn main() {
    // Load CA and identity
    let ca_cert = request::Certificate::from_pem(&fs::read("bundle.pem").unwrap().unwrap());
    let identity_pem = fs::read("svid Bundle.pem").unwrap();
    // svid Bundle.pem should contain the client cert + private key concatenated (PEM format)
    let identity = request::Identity::from_pem(&identity_pem).unwrap();
    let client = request::Client::builder()
        .add_root_certicate(ca_cert)
        .identity identities
        .build().unwrap();
    let resp = client.get("https://service-b)Vibc.cluster.local:8443/hello")
        .send().unwrap();
        println("Response: {}, resp.status());
        let body = resp.text().unwrap();
        println("Body: {}, body");
} 
```

In this snippet: - We trust the internal CA (bundle.pem). - We provide our own cert/key (svid Bundle.pem contains ---BEGIN CERT--- (client cert) ... END CERT--- and ---BEGIN PRIVATE KEY--- ...). - We then make a GET request to service B's URL (here using Kubernetes DNS name and service port). - This will perform mTLS under the hood with rustls. Note: ClientBuilder::identity uses the identity for both client TLS and likely sets SNI from the URL domain. In this case, SNI would be service-b)Vcend.svc.cluster.local - our server's cert doesn't have that as DNS SAN, but our custom server verifier in service B doesn't check DNS, so it's okay. Rustls default might attempt DNS verification unless we disabled it on server side (which we did via verifypeer_name = false). It's somewhat asymmetric but works given we override verification server-side and the client trusts the CA explicitly.

# Verifying it works (Hypothetical sequence):

- Start service A and B with the above config (each has its SPIFFE cert).   
- Service A's logs: "Accepted connection from spiffe://prod.acme.com/ns/background/sa/gateway-sa" for example, when gateway calls.

- If we intentionally use a wrong cert, it should log handshake failed.

# 9.4 Summary of Default Blueprint

For Kubernetes deployments: Use cert-manager's SPIFFE CSI driver to seamlessly provision SPIFFE certificates to each service pod. Use a ClusterIssuer with an internal CA. Configure BRRTRouter-based services with mTLS enabled, pointing to the cert/key paths from CSI. Use trust-manager to distribute the CA bundle to all pods (CSI driver already puts the bundle in the volume). The services will then automatically do mTLS.

At runtime: All services have BRRTRouter_MTLS_ENABLED=true, so they require and verify client certs on every connection. They communicate over service mesh-like security but without a mesh, purely by library enforcement. Let's Encrypt is only used on the public ingress (which could be the BRRTRouter gateway or annginx/ingress controller in front of it).

This architecture achieves end-to-end encryption and authentication: external user -> (LE TLS) -> gateway -> (internal SPIFFE mTLS) -> service -> (if calls another service, again SPIFFE mTLS).

Finally, ensure to document these defaults in the repo's README or operations guide so that developers know how to configure their services for SPIFFE/mTLS.

By implementing the above and following the plan, BRRTRouter and the microcaler platform will enforce a robust zero-trust, SPIFFE-compliant mTLS scheme suitable for high-security environments, with clear operational procedures for maintenance and emergency handling. The combination of short-lived certificates, automatic rotation, and explicit identity verification at the application layer significantly reduces the risk of unauthorized access or credential misuse 6 27 , fulfilling the requirements for finance-grade multi-tenant security. Each service will strictly trust only the identities it's supposed to, and any anomaly can be traced and contained quickly.

This design and plan bring the project in line with industry best-practices for service identity and transport security, leveraging open standards (SPIFFE) and CNCF projects to avoid wheel reinvention, while tailoring the solution to BRTRouter's architecture and the realities of on-prem and hybrid deployments.

1 3 4 6 8 9 17 18 23 24 39 42 55 SPIFFE | SPIFFE Concepts

https://spiffe.io/docs/latest/spiffe-about/spiffe-concepts/

2 7 19 20 33 SPIFFE | SPIFFE Identity and Verifiable Identity Document

https://spiffe.io/docs/latest/spiffe-specs/spiffe-id/

mod.rs

https://github.com/microscalerr/BRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/src/security/spiffe/mod.rs

10 11 12 13 14 15 16 21 22 SPIFFE | SPIRE Concepts

https://spiffe.io/docs/latest/spire-about/spire-concepts/

25 26 27 30 62 63 csi-driver-spiffe - cert-manager Documentation

https://cert-manager.io/docs/usage/csi-driver-spiffe/

28 29 35 36 40 41 43 44 trust-manager - cert-manager Documentation

https://cert-manager.io/docs/trust/trust-manager/

31 From Trust Anchors to SPIFFE IDs: Understanding Linkerd's ...   
https://gtrekter.medium.com/from-trust-anchors-to-spiffe-ids-understanding-linkerds-automated-identity-pipeline-e57a90ce1414   
32 Announcing Linkerd 2.15: Support for VM workloads, native sidecars ...   
https://www.buoyant.io/blog/announcing-linkerd-2-15-vm-workloads-spiffe-identities   
34 Mesh expansion and SPIFFE support arriving in the ... - Linkerd

https://linkerd.io/2023/11/07/linkerd-mesh-expansion/

37 48 Rate Limits - Let's Encrypt

https://letsencrypt.org/docs/rate-limits/

38 47 Let's Encrypt rate limits and alternative solutions - Help - Let's Encrypt Community Support

https://community.letsencrypt.org/t/lets-encrypt-rate-limits-and-alternative-solutions/130780

46 57 61 validation.rs

https://github.com/microScaler/BRRTRouter/blob/88ba14cc2cbc0ba7bb41138d1a4f25d44ed358b3/src/security/spiffe/

validation.rs

49 58 BRTRouter_BLOG_POST.md

https://github.com/microscalerr/BRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/BRTRouter_BLOG_POST.md

52 microservice.py

https://github.com/microcaler/BRRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/tooling/src/

brrtrouter_tooling/bootstrap/microservice.py

59 Questions about setting up tokio-rustls and how to create cents/ keys

https://www.reddit.com/r/rust/comments/7x2fy1/questions_about_settings_up_tokiorustls_and_how_to/

60 Cargo.toml

https://github.com/microScaler/BRTRouter/blob/88ba14cc2cb0ba7bb41138d1a4f25d44ed358b3/Cargo.toml
