#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    id: String,
    group: u8,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fx: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fz: Option<f32>,
}
