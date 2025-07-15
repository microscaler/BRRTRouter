use super::schema::{FieldDef, TypeDefinition, extract_fields, parameter_to_field};
use crate::spec::{ParameterMeta, RouteMeta};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

/// Enhanced template data with validation and documentation support
#[derive(Debug, Clone)]
pub struct EnhancedFieldDef {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    pub value: String,
    pub documentation: Option<String>,
    pub validation_attrs: Option<String>,
}

impl From<FieldDef> for EnhancedFieldDef {
    fn from(field: FieldDef) -> Self {
        Self {
            name: field.name,
            ty: field.ty,
            optional: field.optional,
            value: field.value,
            documentation: None,
            validation_attrs: None,
        }
    }
}

/// Enhanced handler template data with improved structure
#[derive(Debug)]
pub struct EnhancedHandlerTemplateData {
    pub handler_name: String,
    pub request_fields: Vec<EnhancedFieldDef>,
    pub response_fields: Vec<EnhancedFieldDef>,
    pub response_is_array: bool,
    pub response_array_type: String,
    pub imports: Vec<String>,
    pub parameters: Vec<ParameterMeta>,
    pub sse: bool,
    pub spec_path: String,
    pub generation_time: String,
}

/// Enhanced controller template data with improved structure
#[derive(Debug)]
pub struct EnhancedControllerTemplateData {
    pub handler_name: String,
    pub struct_name: String,
    pub response_fields: Vec<EnhancedFieldDef>,
    pub example: String,
    pub has_example: bool,
    pub example_json: String,
    pub response_is_array: bool,
    pub response_array_literal: String,
    pub imports: Vec<String>,
    pub sse: bool,
    pub spec_path: String,
    pub generation_time: String,
}

/// Enhanced types template data with improved structure
#[derive(Debug)]
pub struct EnhancedTypesTemplateData {
    pub types: HashMap<String, EnhancedTypeDefinition>,
    pub spec_path: String,
    pub generation_time: String,
}

/// Enhanced type definition with documentation support
#[derive(Debug, Clone)]
pub struct EnhancedTypeDefinition {
    pub name: String,
    pub fields: Vec<EnhancedFieldDef>,
    pub documentation: Option<String>,
}

impl From<TypeDefinition> for EnhancedTypeDefinition {
    fn from(type_def: TypeDefinition) -> Self {
        Self {
            name: type_def.name,
            fields: type_def.fields.into_iter().map(Into::into).collect(),
            documentation: None,
        }
    }
}

/// Enhanced generator with improved template data generation
pub struct EnhancedGenerator {
    spec_path: String,
    generation_time: String,
}

impl EnhancedGenerator {
    pub fn new(spec_path: impl Into<String>) -> Self {
        let now: DateTime<Utc> = Utc::now();
        Self {
            spec_path: spec_path.into(),
            generation_time: now.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        }
    }

    /// Generate enhanced handler template data
    pub fn generate_handler_data(
        &self,
        handler: &str,
        route: &RouteMeta,
    ) -> EnhancedHandlerTemplateData {
        let mut request_fields = route.request_schema.as_ref().map_or(vec![], extract_fields);
        for param in &route.parameters {
            request_fields.push(parameter_to_field(param));
        }
        
        let response_fields = route.response_schema.as_ref().map_or(vec![], extract_fields);

        let mut imports = BTreeSet::new();
        for field in request_fields.iter().chain(response_fields.iter()) {
            let inner: &str = field.ty
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix(">"))
                .unwrap_or(&field.ty);
            if self.is_named_type(inner) {
                imports.insert(self.to_camel_case(inner));
            }
        }

        let enhanced_request_fields = request_fields
            .into_iter()
            .map(|field| self.enhance_field(field, &route.parameters))
            .collect();

        let enhanced_response_fields = response_fields
            .into_iter()
            .map(|field| self.enhance_field(field, &[]))
            .collect();

        EnhancedHandlerTemplateData {
            handler_name: handler.to_string(),
            request_fields: enhanced_request_fields,
            response_fields: enhanced_response_fields,
            response_is_array: self.is_array_response(&route.response_schema),
            response_array_type: self.get_array_type(&route.response_schema),
            imports: imports.into_iter().collect(),
            parameters: route.parameters.clone(),
            sse: route.sse,
            spec_path: self.spec_path.clone(),
            generation_time: self.generation_time.clone(),
        }
    }

    /// Generate enhanced controller template data
    pub fn generate_controller_data(
        &self,
        handler: &str,
        struct_name: &str,
        route: &RouteMeta,
    ) -> EnhancedControllerTemplateData {
        let response_fields = route.response_schema.as_ref().map_or(vec![], extract_fields);
        
        let enhanced_response_fields: Vec<EnhancedFieldDef> = response_fields
            .into_iter()
            .map(|field| self.enhance_field(field, &[]))
            .collect();

        let mut imports = BTreeSet::new();
        for field in &enhanced_response_fields {
            let inner: &str = field.ty
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix(">"))
                .unwrap_or(&field.ty);
            if self.is_named_type(inner) {
                imports.insert(self.to_camel_case(inner));
            }
        }

        let example_pretty = route.example
            .as_ref()
            .and_then(|v| serde_json::to_string_pretty(v).ok())
            .unwrap_or_default();

        let example_json = if example_pretty.is_empty() {
            String::new()
        } else {
            example_pretty
                .lines()
                .map(|l| format!("/// {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let array_literal = enhanced_response_fields
            .first()
            .map(|f| f.value.clone())
            .unwrap_or_else(|| "vec![Default::default()]".to_string());

        EnhancedControllerTemplateData {
            handler_name: handler.to_string(),
            struct_name: struct_name.to_string(),
            response_fields: enhanced_response_fields,
            example: example_pretty,
            has_example: route.example.is_some(),
            example_json,
            response_is_array: self.is_array_response(&route.response_schema),
            response_array_literal: array_literal,
            imports: imports.into_iter().collect(),
            sse: route.sse,
            spec_path: self.spec_path.clone(),
            generation_time: self.generation_time.clone(),
        }
    }

    /// Generate enhanced types template data
    pub fn generate_types_data(
        &self,
        types: &HashMap<String, TypeDefinition>,
    ) -> EnhancedTypesTemplateData {
        let enhanced_types = types
            .iter()
            .map(|(name, type_def)| {
                let mut enhanced_type: EnhancedTypeDefinition = type_def.clone().into();
                enhanced_type.documentation = Some(format!(
                    "Generated from OpenAPI schema: {name}"
                ));
                (name.clone(), enhanced_type)
            })
            .collect();

        EnhancedTypesTemplateData {
            types: enhanced_types,
            spec_path: self.spec_path.clone(),
            generation_time: self.generation_time.clone(),
        }
    }

    /// Enhance a field with documentation and validation attributes
    fn enhance_field(&self, field: FieldDef, parameters: &[ParameterMeta]) -> EnhancedFieldDef {
        let mut enhanced = EnhancedFieldDef::from(field);
        
        // Add documentation from parameter metadata
        if let Some(param) = parameters.iter().find(|p| p.name == enhanced.name) {
            enhanced.documentation = Some(format!(
                "{} parameter ({})", 
                enhanced.name, 
                param.location.to_string().to_lowercase()
            ));
            
            // Add validation attributes based on parameter schema
            if let Some(schema) = &param.schema {
                enhanced.validation_attrs = self.generate_validation_attrs(schema);
            }
        }

        enhanced
    }

    /// Generate validation attributes from JSON schema
    fn generate_validation_attrs(&self, schema: &Value) -> Option<String> {
        let mut attrs = Vec::new();

        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            attrs.push(format!("minimum = {min}"));
        }
        
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            attrs.push(format!("maximum = {max}"));
        }
        
        if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
            attrs.push(format!("min_length = {min_len}"));
        }
        
        if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            attrs.push(format!("max_length = {max_len}"));
        }
        
        if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
            attrs.push(format!("pattern = \"{pattern}\""));
        }

        if attrs.is_empty() {
            None
        } else {
            Some(format!("#[validate({})]", attrs.join(", ")))
        }
    }

    /// Check if a response schema represents an array
    fn is_array_response(&self, schema: &Option<Value>) -> bool {
        schema
            .as_ref()
            .and_then(|s| s.get("type"))
            .and_then(|t| t.as_str())
            .map(|t| t == "array")
            .unwrap_or(false)
    }

    /// Get the array type from a response schema
    fn get_array_type(&self, schema: &Option<Value>) -> String {
        schema
            .as_ref()
            .and_then(|s| s.get("items"))
            .and_then(|items| items.get("$ref"))
            .and_then(|r| r.as_str())
            .and_then(|r| r.strip_prefix("#/components/schemas/"))
            .map(|name| self.to_camel_case(name))
            .unwrap_or_else(|| "serde_json::Value".to_string())
    }

    /// Check if a type is a named type (not a primitive)
    fn is_named_type(&self, ty: &str) -> bool {
        let primitives = [
            "String", "i32", "i64", "f32", "f64", "bool", "Value", "serde_json::Value",
        ];
        !primitives.contains(&ty) && ty.chars().next().is_some_and(|c| c.is_uppercase())
    }

    /// Convert snake_case to CamelCase
    fn to_camel_case(&self, s: &str) -> String {
        s.split('_')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect()
    }
} 