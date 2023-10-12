use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};

use reqwest::IntoUrl;
use url::form_urlencoded::Serializer;
use url::Url;
use uuid::Uuid;

use graph_error::{IdentityResult, AF};
use graph_extensions::crypto::{secure_random_32, GenPkce, ProofKeyCodeExchange};
use graph_extensions::web::{InteractiveAuthenticator, WebViewOptions};

use crate::auth::{OAuthParameter, OAuthSerializer};
use crate::identity::credentials::app_config::AppConfig;
use crate::identity::{
    Authority, AuthorizationQueryResponse, AuthorizationUrl, AzureCloudInstance, Prompt,
    ResponseMode, ResponseType,
};

/// Get the authorization url required to perform the initial authorization and redirect in the
/// authorization code flow.
///
/// The authorization code flow begins with the client directing the user to the /authorize
/// endpoint.
///
/// The OAuth 2.0 authorization code grant type, or auth code flow, enables a client application
/// to obtain authorized access to protected resources like web APIs. The auth code flow requires
/// a user-agent that supports redirection from the authorization server (the Microsoft identity platform)
/// back to your application. For example, a web browser, desktop, or mobile application operated
/// by a user to sign in to your app and access their data.
///
/// Reference: https://learn.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#request-an-authorization-code
///
/// # Build a confidential client for the authorization code grant.
/// Use [with_authorization_code](crate::identity::ConfidentialClientApplicationBuilder::with_authorization_code) to set the authorization code received from
/// the authorization step, see [Request an authorization code](https://learn.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#request-an-authorization-code)
/// You can use the [AuthCodeAuthorizationUrlParameterBuilder](crate::identity::AuthCodeAuthorizationUrlParameterBuilder)
/// to build the url that the user will be directed to authorize at.
///
/// ```rust
/// fn main() {
///     # use graph_oauth::identity::ConfidentialClientApplication;
///
///     //
///     let client_app = ConfidentialClientApplication::builder("client-id")
///         .with_authorization_code("access-code")
///         .with_client_secret("client-secret")
///         .with_scope(vec!["User.Read"])
///         .build();
/// }
/// ```
#[derive(Clone)]
pub struct AuthCodeAuthorizationUrlParameters {
    pub(crate) app_config: AppConfig,
    pub(crate) response_type: BTreeSet<ResponseType>,
    /// Optional
    /// Specifies how the identity platform should return the requested token to your app.
    ///
    /// Supported values:
    ///
    /// - query: Default when requesting an access token. Provides the code as a query string
    /// parameter on your redirect URI. The query parameter isn't supported when requesting an
    /// ID token by using the implicit flow.
    /// - fragment: Default when requesting an ID token by using the implicit flow.
    /// Also supported if requesting only a code.
    /// - form_post: Executes a POST containing the code to your redirect URI.
    /// Supported when requesting a code.
    pub(crate) response_mode: Option<ResponseMode>,
    pub(crate) nonce: Option<String>,
    pub(crate) state: Option<String>,
    /// Required.
    /// A space-separated list of scopes that you want the user to consent to.
    /// For the /authorize leg of the request, this parameter can cover multiple resources.
    /// This value allows your app to get consent for multiple web APIs you want to call.
    pub(crate) scope: Vec<String>,
    /// Optional
    /// Indicates the type of user interaction that is required. The only valid values at
    /// this time are login, none, consent, and select_account.
    ///
    /// The [Prompt::Login] claim forces the user to enter their credentials on that request,
    /// which negates single sign-on.
    ///
    /// The [Prompt::None] parameter is the opposite, and should be paired with a login_hint to
    /// indicate which user must be signed in. These parameters ensure that the user isn't
    /// presented with any interactive prompt at all. If the request can't be completed silently
    /// via single sign-on, the Microsoft identity platform returns an error. Causes include no
    /// signed-in user, the hinted user isn't signed in, or multiple users are signed in but no
    /// hint was provided.
    ///
    /// The [Prompt::Consent] claim triggers the OAuth consent dialog after the
    /// user signs in. The dialog asks the user to grant permissions to the app.
    ///
    /// Finally, [Prompt::SelectAccount] shows the user an account selector, negating silent SSO but
    /// allowing the user to pick which account they intend to sign in with, without requiring
    /// credential entry. You can't use both login_hint and select_account.
    pub(crate) prompt: Option<Prompt>,
    /// Optional
    /// The realm of the user in a federated directory. This skips the email-based discovery
    /// process that the user goes through on the sign-in page, for a slightly more streamlined
    /// user experience. For tenants that are federated through an on-premises directory
    /// like AD FS, this often results in a seamless sign-in because of the existing login session.
    pub(crate) domain_hint: Option<String>,
    /// Optional
    /// You can use this parameter to pre-fill the username and email address field of the
    /// sign-in page for the user, if you know the username ahead of time. Often, apps use
    /// this parameter during re-authentication, after already extracting the login_hint
    /// optional claim from an earlier sign-in.
    pub(crate) login_hint: Option<String>,
    pub(crate) code_challenge: Option<String>,
    pub(crate) code_challenge_method: Option<String>,
}

impl Debug for AuthCodeAuthorizationUrlParameters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthCodeAuthorizationUrlParameters")
            .field("app_config", &self.app_config)
            .field("scope", &self.scope)
            .field("response_type", &self.response_type)
            .field("response_mode", &self.response_mode)
            .field("prompt", &self.prompt)
            .finish()
    }
}

impl AuthCodeAuthorizationUrlParameters {
    pub fn new<T: AsRef<str>, U: IntoUrl>(
        client_id: T,
        redirect_uri: U,
    ) -> IdentityResult<AuthCodeAuthorizationUrlParameters> {
        let mut response_type = BTreeSet::new();
        response_type.insert(ResponseType::Code);
        let redirect_uri_result = Url::parse(redirect_uri.as_str());

        /*
        AppConfig {
                tenant_id: None,
                client_id: Uuid::try_parse(client_id.as_ref())?,
                authority: Default::default(),
                azure_cloud_instance: Default::default(),
                extra_query_parameters: Default::default(),
                extra_header_parameters: Default::default(),
                redirect_uri: Some(redirect_uri.into_url().or(redirect_uri_result)?),
            },
         */

        Ok(AuthCodeAuthorizationUrlParameters {
            app_config: AppConfig::new_init(
                Uuid::try_parse(client_id.as_ref()).unwrap_or_default(),
                Option::<String>::None,
                Some(redirect_uri.into_url().or(redirect_uri_result)?),
            ),
            response_type,
            response_mode: None,
            nonce: None,
            state: None,
            scope: vec![],
            prompt: None,
            domain_hint: None,
            login_hint: None,
            code_challenge: None,
            code_challenge_method: None,
        })
    }

    pub fn builder<T: AsRef<str>>(client_id: T) -> AuthCodeAuthorizationUrlParameterBuilder {
        AuthCodeAuthorizationUrlParameterBuilder::new(client_id)
    }

    pub fn url(&self) -> IdentityResult<Url> {
        self.url_with_host(&AzureCloudInstance::default())
    }

    pub fn url_with_host(&self, azure_cloud_instance: &AzureCloudInstance) -> IdentityResult<Url> {
        self.authorization_url_with_host(azure_cloud_instance)
    }

    /// Get the nonce.
    ///
    /// This value may be generated automatically by the client and may be useful for users
    /// who want to manually verify that the nonce stored in the client is the same as the
    /// nonce returned in the response from the authorization server.
    /// Verifying the nonce helps mitigate token replay attacks.
    pub fn nonce(&mut self) -> Option<&String> {
        self.nonce.as_ref()
    }

    pub fn interactive_webview_authentication(
        &self,
        interactive_web_view_options: Option<WebViewOptions>,
    ) -> anyhow::Result<AuthorizationQueryResponse> {
        let url_string = self
            .interactive_authentication(interactive_web_view_options)?
            .ok_or(anyhow::Error::msg(
                "Unable to get url from redirect in web view".to_string(),
            ))?;
        dbg!(&url_string);
        /*


        if let Ok(url) = Url::parse(url_string.as_str()) {
            dbg!(&url);

            if let Some(query) = url.query() {
                let response_query: AuthResponseQuery = serde_urlencoded::from_str(query)?;
            }

        }

        let query: HashMap<String, String> =  url.query_pairs().map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect();

                    let code = query.get("code");
                    let id_token = query.get("id_token");
                    let access_token = query.get("access_token");
                    let state = query.get("state");
                    let nonce = query.get("nonce");
                    dbg!(&code, &id_token, &access_token, &state, &nonce);
         */

        let url = Url::parse(&url_string)?;
        let query = url.query().or(url.fragment()).ok_or(AF::msg_err(
            "query | fragment",
            &format!("No query or fragment returned on redirect, url: {url}"),
        ))?;

        let response_query: AuthorizationQueryResponse = serde_urlencoded::from_str(query)?;
        Ok(response_query)
    }
}

mod web_view_authenticator {
    use graph_extensions::web::{InteractiveAuthenticator, InteractiveWebView, WebViewOptions};

    use crate::identity::{AuthCodeAuthorizationUrlParameters, AuthorizationUrl};

    impl InteractiveAuthenticator for AuthCodeAuthorizationUrlParameters {
        fn interactive_authentication(
            &self,
            interactive_web_view_options: Option<WebViewOptions>,
        ) -> anyhow::Result<Option<String>> {
            let uri = self.authorization_url()?;
            let redirect_uri = self.redirect_uri().cloned().unwrap();
            let web_view_options = interactive_web_view_options.unwrap_or_default();
            let _timeout = web_view_options.timeout;
            let (sender, receiver) = std::sync::mpsc::channel();

            std::thread::spawn(move || {
                InteractiveWebView::interactive_authentication(
                    uri,
                    redirect_uri,
                    web_view_options,
                    sender,
                )
                .unwrap();
            });

            let mut iter = receiver.try_iter();
            let mut next = iter.next();
            while next.is_none() {
                next = iter.next();
            }

            Ok(next)
        }
    }
}

impl AuthorizationUrl for AuthCodeAuthorizationUrlParameters {
    fn redirect_uri(&self) -> Option<&Url> {
        self.app_config.redirect_uri.as_ref()
    }

    fn authorization_url(&self) -> IdentityResult<Url> {
        self.authorization_url_with_host(&AzureCloudInstance::default())
    }

    fn authorization_url_with_host(
        &self,
        azure_cloud_instance: &AzureCloudInstance,
    ) -> IdentityResult<Url> {
        let mut serializer = OAuthSerializer::new();

        if let Some(redirect_uri) = self.app_config.redirect_uri.as_ref() {
            if redirect_uri.as_str().trim().is_empty() {
                return AF::result("redirect_uri");
            } else {
                serializer.redirect_uri(redirect_uri.as_str());
            }
        }

        let client_id = self.app_config.client_id.to_string();
        if client_id.is_empty() || self.app_config.client_id.is_nil() {
            return AF::result("client_id");
        }

        if self.scope.is_empty() {
            return AF::result("scope");
        }

        if self.scope.contains(&String::from("openid")) {
            return AF::msg_result(
                "openid",
                "Scope openid is not valid for authorization code - instead use OpenIdCredential",
            );
        }

        serializer
            .client_id(client_id.as_str())
            .extend_scopes(self.scope.clone())
            .authority(azure_cloud_instance, &self.app_config.authority);

        let response_types: Vec<String> =
            self.response_type.iter().map(|s| s.to_string()).collect();

        if response_types.is_empty() {
            serializer.response_type("code");
            if let Some(response_mode) = self.response_mode.as_ref() {
                serializer.response_mode(response_mode.as_ref());
            }
        } else {
            let response_type = response_types.join(" ").trim().to_owned();
            if response_type.is_empty() {
                serializer.response_type("code");
            } else {
                serializer.response_type(response_type);
            }

            // Set response_mode
            if self.response_type.contains(&ResponseType::IdToken) {
                if self.response_mode.is_none() || self.response_mode.eq(&Some(ResponseMode::Query))
                {
                    serializer.response_mode(ResponseMode::Fragment.as_ref());
                } else if let Some(response_mode) = self.response_mode.as_ref() {
                    serializer.response_mode(response_mode.as_ref());
                }
            } else if let Some(response_mode) = self.response_mode.as_ref() {
                serializer.response_mode(response_mode.as_ref());
            }
        }

        if let Some(state) = self.state.as_ref() {
            serializer.state(state.as_str());
        }

        if let Some(prompt) = self.prompt.as_ref() {
            serializer.prompt(prompt.as_ref());
        }

        if let Some(domain_hint) = self.domain_hint.as_ref() {
            serializer.domain_hint(domain_hint.as_str());
        }

        if let Some(login_hint) = self.login_hint.as_ref() {
            serializer.login_hint(login_hint.as_str());
        }

        if let Some(nonce) = self.nonce.as_ref() {
            serializer.nonce(nonce);
        }

        if let Some(code_challenge) = self.code_challenge.as_ref() {
            serializer.code_challenge(code_challenge.as_str());
        }

        if let Some(code_challenge_method) = self.code_challenge_method.as_ref() {
            serializer.code_challenge_method(code_challenge_method.as_str());
        }

        let mut encoder = Serializer::new(String::new());
        serializer.encode_query(
            vec![
                OAuthParameter::ResponseMode,
                OAuthParameter::State,
                OAuthParameter::Prompt,
                OAuthParameter::LoginHint,
                OAuthParameter::DomainHint,
                OAuthParameter::Nonce,
                OAuthParameter::CodeChallenge,
                OAuthParameter::CodeChallengeMethod,
            ],
            vec![
                OAuthParameter::ClientId,
                OAuthParameter::ResponseType,
                OAuthParameter::RedirectUri,
                OAuthParameter::Scope,
            ],
            &mut encoder,
        )?;

        let authorization_url = serializer
            .get(OAuthParameter::AuthorizationUrl)
            .ok_or(AF::msg_internal_err("authorization_url"))?;
        let mut url = Url::parse(authorization_url.as_str())?;
        url.set_query(Some(encoder.finish().as_str()));
        Ok(url)
    }
}

#[derive(Clone)]
pub struct AuthCodeAuthorizationUrlParameterBuilder {
    parameters: AuthCodeAuthorizationUrlParameters,
}

impl AuthCodeAuthorizationUrlParameterBuilder {
    pub fn new<T: AsRef<str>>(client_id: T) -> AuthCodeAuthorizationUrlParameterBuilder {
        let mut response_type = BTreeSet::new();
        response_type.insert(ResponseType::Code);
        AuthCodeAuthorizationUrlParameterBuilder {
            parameters: AuthCodeAuthorizationUrlParameters {
                app_config: AppConfig::new_with_client_id(client_id.as_ref()),
                response_mode: None,
                response_type,
                nonce: None,
                state: None,
                scope: vec![],
                prompt: None,
                domain_hint: None,
                login_hint: None,
                code_challenge: None,
                code_challenge_method: None,
            },
        }
    }

    pub(crate) fn new_with_app_config(
        app_config: AppConfig,
    ) -> AuthCodeAuthorizationUrlParameterBuilder {
        let mut response_type = BTreeSet::new();
        response_type.insert(ResponseType::Code);
        AuthCodeAuthorizationUrlParameterBuilder {
            parameters: AuthCodeAuthorizationUrlParameters {
                app_config,
                response_mode: None,
                response_type,
                nonce: None,
                state: None,
                scope: vec![],
                prompt: None,
                domain_hint: None,
                login_hint: None,
                code_challenge: None,
                code_challenge_method: None,
            },
        }
    }

    pub fn with_redirect_uri<U: IntoUrl>(&mut self, redirect_uri: U) -> &mut Self {
        self.parameters.app_config.redirect_uri = Some(redirect_uri.into_url().unwrap());
        self
    }

    pub fn with_client_id<T: AsRef<str>>(&mut self, client_id: T) -> &mut Self {
        self.parameters.app_config.client_id =
            Uuid::try_parse(client_id.as_ref()).expect("Invalid Client Id - Must be a Uuid ");
        self
    }

    /// Convenience method. Same as calling [with_authority(Authority::TenantId("tenant_id"))]
    pub fn with_tenant<T: AsRef<str>>(&mut self, tenant: T) -> &mut Self {
        self.parameters.app_config.authority = Authority::TenantId(tenant.as_ref().to_owned());
        self
    }

    pub fn with_authority<T: Into<Authority>>(&mut self, authority: T) -> &mut Self {
        self.parameters.app_config.authority = authority.into();
        self
    }

    /// Default is code. Must include code for the authorization code flow.
    /// Can also include id_token or token if using the hybrid flow.
    pub fn with_response_type<I: IntoIterator<Item = ResponseType>>(
        &mut self,
        response_type: I,
    ) -> &mut Self {
        self.parameters
            .response_type
            .extend(response_type.into_iter());
        self
    }

    /// Specifies how the identity platform should return the requested token to your app.
    ///
    /// Supported values:
    ///
    /// - **query**: Default when requesting an access token. Provides the code as a query string
    ///     parameter on your redirect URI. The query parameter is not supported when requesting an
    ///     ID token by using the implicit flow.
    /// - **fragment**: Default when requesting an ID token by using the implicit flow.
    ///     Also supported if requesting only a code.
    /// - **form_post**: Executes a POST containing the code to your redirect URI.
    ///     Supported when requesting a code.
    pub fn with_response_mode(&mut self, response_mode: ResponseMode) -> &mut Self {
        self.parameters.response_mode = Some(response_mode);
        self
    }

    /// A value included in the request, generated by the app, that is included in the
    /// resulting id_token as a claim. The app can then verify this value to mitigate token
    /// replay attacks. The value is typically a randomized, unique string that can be used
    /// to identify the origin of the request.
    pub fn with_nonce<T: AsRef<str>>(&mut self, nonce: T) -> &mut Self {
        self.parameters.nonce = Some(nonce.as_ref().to_owned());
        self
    }

    /// A value included in the request, generated by the app, that is included in the
    /// resulting id_token as a claim. The app can then verify this value to mitigate token
    /// replay attacks. The value is typically a randomized, unique string that can be used
    /// to identify the origin of the request.
    ///
    /// The nonce is generated in the same way as generating a PKCE.
    ///
    /// Internally this method uses the Rust ring cyrpto library to
    /// generate a secure random 32-octet sequence that is base64 URL
    /// encoded (no padding). This sequence is hashed using SHA256 and
    /// base64 URL encoded (no padding) resulting in a 43-octet URL safe string.
    #[doc(hidden)]
    pub(crate) fn with_nonce_generated(&mut self) -> IdentityResult<&mut Self> {
        self.parameters.nonce = Some(secure_random_32()?);
        Ok(self)
    }

    pub fn with_state<T: AsRef<str>>(&mut self, state: T) -> &mut Self {
        self.parameters.state = Some(state.as_ref().to_owned());
        self
    }

    /// Required.
    /// A space-separated list of scopes that you want the user to consent to.
    /// For the /authorize leg of the request, this parameter can cover multiple resources.
    /// This value allows your app to get consent for multiple web APIs you want to call.
    pub fn with_scope<T: ToString, I: IntoIterator<Item = T>>(&mut self, scope: I) -> &mut Self {
        self.parameters.scope.extend(
            scope
                .into_iter()
                .map(|s| s.to_string())
                .map(|s| s.trim().to_owned()),
        );
        self
    }

    /// Adds the `offline_access` scope parameter which tells the authorization server
    /// to include a refresh token in the redirect uri query.
    pub fn with_offline_access(&mut self) -> &mut Self {
        self.parameters
            .scope
            .extend(vec!["offline_access".to_owned()]);
        self
    }

    /// Indicates the type of user interaction that is required. Valid values are login, none,
    /// consent, and select_account.
    ///
    /// - **prompt=login** forces the user to enter their credentials on that request, negating single-sign on.
    /// - **prompt=none** is the opposite. It ensures that the user isn't presented with any interactive prompt.
    ///     If the request can't be completed silently by using single-sign on, the Microsoft identity platform returns an interaction_required error.
    /// - **prompt=consent** triggers the OAuth consent dialog after the user signs in, asking the user to
    ///     grant permissions to the app.
    /// - **prompt=select_account** interrupts single sign-on providing account selection experience
    ///     listing all the accounts either in session or any remembered account or an option to choose to use a different account altogether.
    pub fn with_prompt(&mut self, prompt: Prompt) -> &mut Self {
        self.parameters.prompt = Some(prompt);
        self
    }

    pub fn with_domain_hint<T: AsRef<str>>(&mut self, domain_hint: T) -> &mut Self {
        self.parameters.domain_hint = Some(domain_hint.as_ref().to_owned());
        self
    }

    pub fn with_login_hint<T: AsRef<str>>(&mut self, login_hint: T) -> &mut Self {
        self.parameters.login_hint = Some(login_hint.as_ref().to_owned());
        self
    }

    /// Used to secure authorization code grants by using Proof Key for Code Exchange (PKCE).
    /// Required if code_challenge_method is included.
    pub fn with_code_challenge<T: AsRef<str>>(&mut self, code_challenge: T) -> &mut Self {
        self.parameters.code_challenge = Some(code_challenge.as_ref().to_owned());
        self
    }

    /// The method used to encode the code_verifier for the code_challenge parameter.
    /// This SHOULD be S256, but the spec allows the use of plain if the client can't support SHA256.
    ///
    /// If excluded, code_challenge is assumed to be plaintext if code_challenge is included.
    /// The Microsoft identity platform supports both plain and S256.
    pub fn with_code_challenge_method<T: AsRef<str>>(
        &mut self,
        code_challenge_method: T,
    ) -> &mut Self {
        self.parameters.code_challenge_method = Some(code_challenge_method.as_ref().to_owned());
        self
    }

    /// Sets the code_challenge and code_challenge_method using the [ProofKeyCodeExchange]
    /// Callers should keep the [ProofKeyCodeExchange] and provide it to the credential
    /// builder in order to set the client verifier and request an access token.
    pub fn with_pkce(&mut self, proof_key_for_code_exchange: &ProofKeyCodeExchange) -> &mut Self {
        self.with_code_challenge(proof_key_for_code_exchange.code_challenge.as_str());
        self.with_code_challenge_method(proof_key_for_code_exchange.code_challenge_method.as_str());
        self
    }

    pub fn build(&self) -> AuthCodeAuthorizationUrlParameters {
        self.parameters.clone()
    }

    pub fn url(&self) -> IdentityResult<Url> {
        self.parameters.url()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serialize_uri() {
        let authorizer = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .build();

        let url_result = authorizer.url();
        assert!(url_result.is_ok());
    }

    #[test]
    fn url_with_host() {
        let authorizer = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .build();

        let url_result = authorizer.url_with_host(&AzureCloudInstance::AzureGermany);
        assert!(url_result.is_ok());
    }

    #[test]
    fn response_mode_set() {
        let url = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .with_response_type(ResponseType::IdToken)
            .url()
            .unwrap();

        let query = url.query().unwrap();
        dbg!(query);
        assert!(query.contains("response_mode=fragment"));
        assert!(query.contains("response_type=code+id_token"));
    }

    #[test]
    fn response_mode_not_set() {
        let url = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .url()
            .unwrap();

        let query = url.query().unwrap();
        assert!(!query.contains("response_mode"));
        assert!(query.contains("response_type=code"));
    }

    #[test]
    fn multi_response_type_set() {
        let url = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .with_response_mode(ResponseMode::FormPost)
            .with_response_type(vec![ResponseType::IdToken, ResponseType::Code])
            .url()
            .unwrap();

        let query = url.query().unwrap();
        assert!(query.contains("response_mode=form_post"));
        assert!(query.contains("response_type=code+id_token"));
    }

    #[test]
    fn generate_nonce() {
        let url = AuthCodeAuthorizationUrlParameters::builder(Uuid::new_v4().to_string())
            .with_redirect_uri("https://localhost:8080")
            .with_scope(["read", "write"])
            .with_response_type(vec![ResponseType::Code, ResponseType::IdToken])
            .with_nonce_generated()
            .unwrap()
            .url()
            .unwrap();

        let query = url.query().unwrap();
        assert!(query.contains("response_mode=fragment"));
        assert!(query.contains("response_type=code+id_token"));
        assert!(query.contains("nonce"));
    }
}
