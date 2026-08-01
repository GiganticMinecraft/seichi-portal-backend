-- SeaORM 時代の本番データベーススキーマを再現するフィクスチャ。
-- CI の migration-convergence-check ジョブで使用する:
-- このスキーマへ全マイグレーションを適用した結果が、空のデータベースへ適用した
-- 結果と同一スキーマに収束することを検証する。
--
-- スキーマは 2026-08-01 時点の本番 DDL (SeaORM 由来のテーブルのみ) から採取したもの。
-- INSERT はバックフィルの検証用の合成データであり、実データは含まない。

CREATE TABLE `users` (
  `id` char(36) NOT NULL,
  `name` varchar(16) NOT NULL,
  `role` enum('ADMINISTRATOR','STANDARD_USER') NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `form_meta_data` (
  `id` char(36) NOT NULL,
  `title` text NOT NULL,
  `description` text NOT NULL,
  `visibility` enum('PUBLIC','PRIVATE') NOT NULL DEFAULT 'PRIVATE',
  `answer_visibility` enum('PUBLIC','PRIVATE') NOT NULL DEFAULT 'PRIVATE',
  `created_at` datetime NOT NULL DEFAULT current_timestamp(),
  `created_by` char(36) NOT NULL,
  `updated_at` datetime NOT NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),
  `updated_by` char(36) NOT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_form_meta_data_created_by` (`created_by`),
  KEY `fk_form_meta_data_updated_by` (`updated_by`),
  CONSTRAINT `fk_form_meta_data_created_by` FOREIGN KEY (`created_by`) REFERENCES `users` (`id`),
  CONSTRAINT `fk_form_meta_data_updated_by` FOREIGN KEY (`updated_by`) REFERENCES `users` (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `form_questions` (
  `question_id` int(11) NOT NULL AUTO_INCREMENT,
  `form_id` char(36) NOT NULL,
  `title` text NOT NULL,
  `description` text DEFAULT NULL,
  `question_type` enum('TEXT','SINGLE','MULTIPLE') DEFAULT NULL,
  `is_required` tinyint(1) DEFAULT 0,
  PRIMARY KEY (`question_id`),
  KEY `fk_form_questions_form_id` (`form_id`),
  CONSTRAINT `fk_form_questions_form_id` FOREIGN KEY (`form_id`) REFERENCES `form_meta_data` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `form_choices` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `question_id` int(11) NOT NULL,
  `choice` text NOT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_form_choices_question_id` (`question_id`),
  CONSTRAINT `fk_form_choices_question_id` FOREIGN KEY (`question_id`) REFERENCES `form_questions` (`question_id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `answers` (
  `id` char(36) NOT NULL,
  `form_id` char(36) NOT NULL,
  `user` char(36) NOT NULL,
  `title` text DEFAULT NULL,
  `timestamp` timestamp NULL DEFAULT current_timestamp(),
  PRIMARY KEY (`id`),
  KEY `fk_answers_form_id` (`form_id`),
  KEY `fk_answers_user` (`user`),
  CONSTRAINT `fk_answers_form_id` FOREIGN KEY (`form_id`) REFERENCES `form_meta_data` (`id`) ON DELETE CASCADE,
  CONSTRAINT `fk_answers_user` FOREIGN KEY (`user`) REFERENCES `users` (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `real_answers` (
  `id` char(36) NOT NULL,
  `answer_id` char(36) NOT NULL,
  `question_id` int(11) NOT NULL,
  `answer` text NOT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_real_answers_answer_id` (`answer_id`),
  KEY `fk_real_answers_quesiton_id` (`question_id`),
  CONSTRAINT `fk_real_answers_answer_id` FOREIGN KEY (`answer_id`) REFERENCES `answers` (`id`),
  CONSTRAINT `fk_real_answers_quesiton_id` FOREIGN KEY (`question_id`) REFERENCES `form_questions` (`question_id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `response_period` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `form_id` char(36) NOT NULL,
  `start_at` datetime DEFAULT NULL,
  `end_at` datetime DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_response_period_form_id` (`form_id`),
  CONSTRAINT `fk_response_period_form_id` FOREIGN KEY (`form_id`) REFERENCES `form_meta_data` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `default_answer_titles` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `form_id` char(36) NOT NULL,
  `title` text DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_default_answer_titles_form_id` (`form_id`),
  CONSTRAINT `fk_default_answer_titles_form_id` FOREIGN KEY (`form_id`) REFERENCES `form_meta_data` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `form_webhooks` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `form_id` char(36) NOT NULL,
  `url` text NOT NULL,
  PRIMARY KEY (`id`),
  KEY `fk_form_webhooks_form_id` (`form_id`),
  CONSTRAINT `fk_form_webhooks_form_id` FOREIGN KEY (`form_id`) REFERENCES `form_meta_data` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE `seaql_migrations` (
  `version` varchar(255) NOT NULL,
  `applied_at` bigint(20) NOT NULL,
  PRIMARY KEY (`version`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

-- ---------------------------------------------------------------------------
-- バックフィル検証用の合成データ
-- ---------------------------------------------------------------------------

INSERT INTO `users` VALUES
  ('00000000-0000-0000-0000-0000000000aa', 'alice', 'ADMINISTRATOR');

INSERT INTO `form_meta_data` (`id`, `title`, `description`, `visibility`, `answer_visibility`, `created_by`, `updated_by`) VALUES
  ('00000000-0000-0000-0000-0000000000f1', 'テストフォーム', '説明', 'PUBLIC', 'PUBLIC',
   '00000000-0000-0000-0000-0000000000aa', '00000000-0000-0000-0000-0000000000aa');

INSERT INTO `form_questions` (`form_id`, `title`, `description`, `question_type`, `is_required`) VALUES
  ('00000000-0000-0000-0000-0000000000f1', '質問1', NULL, 'TEXT', 1),
  ('00000000-0000-0000-0000-0000000000f1', '質問2', NULL, 'SINGLE', 0),
  ('00000000-0000-0000-0000-0000000000f1', '質問3', NULL, NULL, 0);

INSERT INTO `form_choices` (`question_id`, `choice`) VALUES
  (2, '選択肢A'),
  (2, '選択肢B'),
  (2, '選択肢C');

INSERT INTO `answers` (`id`, `form_id`, `user`, `title`) VALUES
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000f1',
   '00000000-0000-0000-0000-0000000000aa', '回答1');

INSERT INTO `real_answers` (`id`, `answer_id`, `question_id`, `answer`) VALUES
  ('00000000-0000-0000-0000-0000000000e1', '00000000-0000-0000-0000-0000000000a1', 1, '自由記述の回答');

INSERT INTO `response_period` (`form_id`, `start_at`, `end_at`) VALUES
  ('00000000-0000-0000-0000-0000000000f1', '2026-01-01 00:00:00', '2026-12-31 23:59:59');

INSERT INTO `default_answer_titles` (`form_id`, `title`) VALUES
  ('00000000-0000-0000-0000-0000000000f1', 'デフォルトタイトル');

INSERT INTO `form_webhooks` (`form_id`, `url`) VALUES
  ('00000000-0000-0000-0000-0000000000f1', 'https://example.invalid/hook');

INSERT INTO `seaql_migrations` VALUES
  ('m20220101_000001_create_table', 1650000000);
