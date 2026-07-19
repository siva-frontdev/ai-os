use crate::memory_object::MemoryObject;
use crate::relationship::Relationship;
use crate::types::{Checksum, MemoryImportance, MemoryPriority};
use crate::MemoryError;

/// Validation trait for types that can be validated.
///
/// Implementors provide their own validation logic.
/// Returns `Ok(())` on success or `Err(MemoryError::ValidationError)` on failure.
pub trait Validate {
    /// Validate the object's invariants.
    fn validate(&self) -> Result<(), MemoryError>;
}

impl Validate for MemoryObject {
    fn validate(&self) -> Result<(), MemoryError> {
        // Validate importance range
        MemoryImportance::new(self.importance)?;

        // Validate priority (already constrained by u8)
        let _ = MemoryPriority::new(self.priority);

        // Validate relationships
        for rel in &self.relationships {
            rel.validate()?;
        }

        // Validate that timestamp is not in the far future (optional sanity check)
        let now = crate::types::Timestamp::now();
        if self.timestamp > now && self.timestamp.as_nanos() - now.as_nanos() > 3_600_000_000_000_000_000 {
            // 1 hour in the future as nanoseconds, allow some clock skew
            return Err(MemoryError::ValidationError(
                "timestamp is unreasonably far in the future".into(),
            ));
        }

        // Validate expiration is after creation
        if let Some(expiration) = self.expiration {
            if expiration < self.timestamp {
                return Err(MemoryError::ValidationError(
                    "expiration must be after timestamp".into(),
                ));
            }
        }

        // Validate content_type is not empty
        if self.content_type.is_empty() {
            return Err(MemoryError::ValidationError(
                "content_type must not be empty".into(),
            ));
        }

        Ok(())
    }
}

impl Validate for Relationship {
    fn validate(&self) -> Result<(), MemoryError> {
        // Validate weight range
        if !(0.0..=1.0).contains(&self.weight) {
            return Err(MemoryError::ValidationError(format!(
                "relationship weight must be between 0.0 and 1.0, got {}",
                self.weight
            )));
        }
        Ok(())
    }
}

impl Validate for Checksum {
    fn validate(&self) -> Result<(), MemoryError> {
        // A checksum is always valid as a byte array
        Ok(())
    }
}

/// Marker trait for types that are serializable to/from JSON bytes.
pub trait ToJsonBytes: Sized {
    /// Serialize to JSON bytes.
    fn to_json_bytes(&self) -> Result<Vec<u8>, MemoryError>;
    /// Deserialize from JSON bytes.
    fn from_json_bytes(bytes: &[u8]) -> Result<Self, MemoryError>;
}

impl ToJsonBytes for MemoryObject {
    fn to_json_bytes(&self) -> Result<Vec<u8>, MemoryError> {
        serde_json::to_vec(self).map_err(|e| MemoryError::SerializationError(e.to_string()))
    }

    fn from_json_bytes(bytes: &[u8]) -> Result<Self, MemoryError> {
        serde_json::from_slice(bytes).map_err(|e| MemoryError::SerializationError(e.to_string()))
    }
}

/// Trait for types that can compute their own checksum.
pub trait Checksumable {
    /// Compute and return the checksum.
    fn compute_checksum(&self) -> Checksum;
}

impl Checksumable for MemoryObject {
    fn compute_checksum(&self) -> Checksum {
        let data = match serde_json::to_vec(&self.content) {
            Ok(bytes) => bytes,
            Err(_) => self.content.clone(),
        };
        Checksum::compute(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_validate_valid_memory_object() {
        let obj = MemoryObject::builder()
            .content_type("text/plain")
            .build();
        assert!(obj.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_content_type() {
        let obj = MemoryObject::builder()
            .content_type("")
            .build();
        assert!(obj.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_importance() {
        let mut obj = MemoryObject::builder().build();
        obj.importance = 1.5;
        assert!(obj.validate().is_err());
    }

    #[test]
    fn test_validate_relationship_weight() {
        let rel = Relationship {
            target_id: MemoryId::new(),
            relation_type: RelationType::References,
            weight: 1.5,
        };
        assert!(rel.validate().is_err());

        let rel = Relationship {
            target_id: MemoryId::new(),
            relation_type: RelationType::References,
            weight: 0.5,
        };
        assert!(rel.validate().is_ok());
    }

    #[test]
    fn test_to_json_bytes_roundtrip() {
        let obj = MemoryObject::builder().build();
        let bytes = obj.to_json_bytes().unwrap();
        let restored = MemoryObject::from_json_bytes(&bytes).unwrap();
        assert_eq!(obj.id, restored.id);
        assert_eq!(obj.version, restored.version);
        assert_eq!(obj.content_type, restored.content_type);
    }

    #[test]
    fn test_checksum_verification() {
        let obj = MemoryObject::builder().build();
        let cs = obj.compute_checksum();
        let data = serde_json::to_vec(&obj.content).unwrap_or_else(|_| obj.content.clone());
        assert!(cs.verify(&data));
    }

    #[test]
    fn test_validate_invalid_expiration() {
        let ts = Timestamp::now();
        let obj = MemoryObject::builder()
            .timestamp(ts)
            .expiration(Some(Timestamp::from_nanos(ts.as_nanos() - 1_000_000_000)))
            .build();
        assert!(obj.validate().is_err());
    }
}
