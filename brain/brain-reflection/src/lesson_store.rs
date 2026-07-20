use crate::errors::{ReflectionError, ReflectionResult};
use crate::types::Lesson;
use brain_core::ids::{GoalId, LessonId};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug)]
pub struct LessonStore {
    lessons: RwLock<HashMap<LessonId, Lesson>>,
}

impl LessonStore {
    pub fn new() -> Self {
        Self { lessons: RwLock::new(HashMap::new()) }
    }

    pub fn store(&self, lesson: Lesson) -> ReflectionResult<()> {
        let mut lessons = self.lessons.write().map_err(|e| ReflectionError::Internal(e.to_string()))?;
        lessons.insert(lesson.lesson_id, lesson);
        Ok(())
    }

    pub fn get(&self, lesson_id: &LessonId) -> ReflectionResult<Lesson> {
        let lessons = self.lessons.read().map_err(|e| ReflectionError::Internal(e.to_string()))?;
        lessons.get(lesson_id).cloned().ok_or_else(|| ReflectionError::NoLessons(format!("lesson {:?} not found", lesson_id)))
    }

    pub fn list_for_goal(&self, goal_id: &GoalId) -> ReflectionResult<Vec<Lesson>> {
        let lessons = self.lessons.read().map_err(|e| ReflectionError::Internal(e.to_string()))?;
        Ok(lessons.values().filter(|l| l.goal_id == *goal_id).cloned().collect())
    }

    pub fn list_all(&self) -> ReflectionResult<Vec<Lesson>> {
        let lessons = self.lessons.read().map_err(|e| ReflectionError::Internal(e.to_string()))?;
        Ok(lessons.values().cloned().collect())
    }

    pub fn increment_applied(&self, lesson_id: &LessonId) -> ReflectionResult<()> {
        let mut lessons = self.lessons.write().map_err(|e| ReflectionError::Internal(e.to_string()))?;
        if let Some(lesson) = lessons.get_mut(lesson_id) {
            lesson.applied_count += 1;
            Ok(())
        } else {
            Err(ReflectionError::NoLessons(format!("lesson {:?} not found", lesson_id)))
        }
    }
}

impl Default for LessonStore {
    fn default() -> Self { Self::new() }
}
