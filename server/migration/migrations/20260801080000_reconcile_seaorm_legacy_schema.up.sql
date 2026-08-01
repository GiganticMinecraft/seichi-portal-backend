-- SeaORM 時代に構築されたデータベースを現行スキーマへ是正するマイグレーション。
--
-- 背景:
--   本番データベースは SeaORM の migrator で構築された後に sqlx へ移行したが、
--   初期マイグレーション (20220101000001_create_table) は CREATE TABLE IF NOT EXISTS で
--   書かれているため、既存テーブルには一切変更が適用されず、スキーマドリフトが発生した。
--   このマイグレーションはドリフトした全テーブルを初期マイグレーションと同一の形へ収束させる。
--
-- 設計方針:
--   - 冪等: MySQL/MariaDB の DDL はロールバック不能なため、途中失敗からの再実行に耐えるよう
--     全ステートメントを「適用済みなら no-op」になるように書く。
--   - 新規データベース (初期マイグレーションだけで構築された DB) では全体が no-op になる。
--   - 条件付き DDL を表現できない箇所は information_schema を参照する動的 SQL
--     (SET / PREPARE / EXECUTE) で分岐する。
--   - 収束の同一性は CI の migration-convergence-check ジョブで検証される。

-- ============================================================================
-- Phase A: form_meta_data — 欠落カラムの追加とレガシーテーブルからのバックフィル
-- ============================================================================

ALTER TABLE form_meta_data
    ADD COLUMN IF NOT EXISTS allow_temporary_answers BOOL NOT NULL DEFAULT FALSE AFTER visibility,
    ADD COLUMN IF NOT EXISTS hide_author BOOL NOT NULL DEFAULT FALSE AFTER answer_visibility,
    ADD COLUMN IF NOT EXISTS acceptance_period_start_at DATETIME AFTER hide_author,
    ADD COLUMN IF NOT EXISTS acceptance_period_end_at DATETIME AFTER acceptance_period_start_at,
    ADD COLUMN IF NOT EXISTS default_answer_title TEXT AFTER acceptance_period_end_at;

-- レガシーテーブルが存在しない (新規構築の) データベースでも後続の UPDATE が
-- パースエラーにならないよう、空のテーブルを用意してから参照する。
CREATE TABLE IF NOT EXISTS response_period(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    start_at DATETIME,
    end_at DATETIME
);

CREATE TABLE IF NOT EXISTS default_answer_titles(
    id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    form_id CHAR(36) NOT NULL,
    title TEXT
);

-- 回答受付期間: form ごとに最新の response_period 行を採用する
UPDATE form_meta_data f
JOIN (
    SELECT rp.form_id, rp.start_at, rp.end_at
    FROM response_period rp
    JOIN (SELECT form_id, MAX(id) AS max_id FROM response_period GROUP BY form_id) latest
        ON latest.form_id = rp.form_id AND latest.max_id = rp.id
) p ON p.form_id = f.id
SET f.acceptance_period_start_at = p.start_at,
    f.acceptance_period_end_at   = p.end_at
WHERE f.acceptance_period_start_at IS NULL AND f.acceptance_period_end_at IS NULL;

-- デフォルト回答タイトル: form ごとに最新の default_answer_titles 行を採用する
UPDATE form_meta_data f
JOIN (
    SELECT dat.form_id, dat.title
    FROM default_answer_titles dat
    JOIN (SELECT form_id, MAX(id) AS max_id FROM default_answer_titles GROUP BY form_id) latest
        ON latest.form_id = dat.form_id AND latest.max_id = dat.id
) d ON d.form_id = f.id
SET f.default_answer_title = d.title
WHERE f.default_answer_title IS NULL;

-- ============================================================================
-- Phase B: answers — 匿名回答 / Redmine 移行対応カラムの追加
-- ============================================================================

-- 既存行はすべて認証ユーザーの回答なので DEFAULT で AUTHENTICATED_USER を埋める
ALTER TABLE answers
    ADD COLUMN IF NOT EXISTS author_type ENUM('AUTHENTICATED_USER', 'TEMPORARY_USER', 'IMPORTED_FROM_REDMINE')
        NOT NULL DEFAULT 'AUTHENTICATED_USER' AFTER form_id,
    ADD COLUMN IF NOT EXISTS temporary_user_id CHAR(36) AFTER user,
    ADD COLUMN IF NOT EXISTS redmine_user_id BIGINT AFTER temporary_user_id,
    ADD COLUMN IF NOT EXISTS redmine_author_name TEXT AFTER redmine_user_id,
    ADD COLUMN IF NOT EXISTS publication ENUM('PUBLIC', 'PRIVATE') NOT NULL DEFAULT 'PUBLIC' AFTER title;

ALTER TABLE answers MODIFY COLUMN user CHAR(36);

-- 初期マイグレーションの author_type に DEFAULT はないため、埋め終わったら外す
SET @stmt := IF(
    (SELECT COLUMN_DEFAULT IS NOT NULL FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'answers' AND COLUMN_NAME = 'author_type'),
    'ALTER TABLE answers ALTER COLUMN author_type DROP DEFAULT',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

ALTER TABLE answers
    ADD CONSTRAINT fk_answers_temporary_user_id
        FOREIGN KEY IF NOT EXISTS (temporary_user_id) REFERENCES temporary_users(id);

-- 初期マイグレーション由来の無名 CHECK (自動命名 CONSTRAINT_1) を明示名へ統一する
ALTER TABLE answers DROP CONSTRAINT IF EXISTS CONSTRAINT_1;
ALTER TABLE answers ADD CONSTRAINT IF NOT EXISTS chk_answers_author CHECK (
    (author_type = 'AUTHENTICATED_USER' AND user IS NOT NULL AND temporary_user_id IS NULL
        AND redmine_user_id IS NULL AND redmine_author_name IS NULL)
    OR (author_type = 'TEMPORARY_USER' AND user IS NULL AND temporary_user_id IS NOT NULL
        AND redmine_user_id IS NULL AND redmine_author_name IS NULL)
    OR (author_type = 'IMPORTED_FROM_REDMINE' AND user IS NULL AND temporary_user_id IS NULL
        AND redmine_author_name IS NOT NULL)
);

-- ============================================================================
-- Phase C: form_questions / form_choices / real_answers —
--          question_id の INT AUTO_INCREMENT → CHAR(36) UUID 変換
--
-- レガシースキーマのときだけ実行される。各ステップは直前に information_schema で
-- 進行状態を確認するため、途中失敗後の再実行でも壊れない。
-- ============================================================================

-- C-0: INT id の並び順が残っているうちに position をバックフィルする
ALTER TABLE form_questions ADD COLUMN IF NOT EXISTS position SMALLINT UNSIGNED AFTER form_id;
UPDATE form_questions fq
JOIN (
    SELECT question_id, ROW_NUMBER() OVER (PARTITION BY form_id ORDER BY question_id) AS rn
    FROM form_questions
) t ON t.question_id = fq.question_id
SET fq.position = t.rn
WHERE fq.position IS NULL;

ALTER TABLE form_choices ADD COLUMN IF NOT EXISTS position SMALLINT UNSIGNED;
UPDATE form_choices fc
JOIN (
    SELECT id, ROW_NUMBER() OVER (PARTITION BY question_id ORDER BY id) AS rn
    FROM form_choices
) t ON t.id = fc.id
SET fc.position = t.rn
WHERE fc.position IS NULL;

-- C-1: form_questions に UUID カラムを追加して採番する
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_uuid') = 0,
    'ALTER TABLE form_questions ADD COLUMN question_uuid CHAR(36)',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_uuid') = 1,
    'UPDATE form_questions SET question_uuid = UUID() WHERE question_uuid IS NULL',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

-- C-2: 子テーブルへ UUID を伝播する (INT の結合キーが残っているうちに行う)
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_uuid') = 0,
    'ALTER TABLE form_choices ADD COLUMN question_uuid CHAR(36)',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_uuid') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1,
    'UPDATE form_choices fc JOIN form_questions fq ON fc.question_id = fq.question_id
       SET fc.question_uuid = fq.question_uuid WHERE fc.question_uuid IS NULL',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_uuid') = 0,
    'ALTER TABLE real_answers ADD COLUMN question_uuid CHAR(36)',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_uuid') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1,
    'UPDATE real_answers ra JOIN form_questions fq ON ra.question_id = fq.question_id
       SET ra.question_uuid = fq.question_uuid WHERE ra.question_uuid IS NULL',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

-- C-3: 子テーブルの旧 FK と INT カラムを除去する
-- (UUID 未伝播の行があると後段の NOT NULL 化が失敗して止まるため、データは失われない)
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1,
    'ALTER TABLE form_choices DROP FOREIGN KEY IF EXISTS fk_form_choices_question_id, DROP COLUMN question_id',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

-- 本番のレガシー FK は fk_real_answers_quesiton_id と typo しているため両方の名前を落とす
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1,
    'ALTER TABLE real_answers DROP FOREIGN KEY IF EXISTS fk_real_answers_quesiton_id,
       DROP FOREIGN KEY IF EXISTS fk_real_answers_question_id, DROP COLUMN question_id',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

-- C-4: form_questions の主キーを INT から UUID へ差し替える
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int'
       AND EXTRA LIKE '%auto_increment%') = 1,
    'ALTER TABLE form_questions MODIFY COLUMN question_id INT NOT NULL',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1
    AND (SELECT COUNT(*) FROM information_schema.TABLE_CONSTRAINTS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND CONSTRAINT_TYPE = 'PRIMARY KEY') = 1,
    'ALTER TABLE form_questions DROP PRIMARY KEY',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_id' AND DATA_TYPE = 'int') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_uuid') = 1,
    'ALTER TABLE form_questions DROP COLUMN question_id',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_uuid') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND COLUMN_NAME = 'question_id') = 0,
    'ALTER TABLE form_questions CHANGE COLUMN question_uuid question_id CHAR(36) NOT NULL FIRST',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.TABLE_CONSTRAINTS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_questions'
       AND CONSTRAINT_TYPE = 'PRIMARY KEY') = 0,
    'ALTER TABLE form_questions ADD PRIMARY KEY (question_id)',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

-- C-5: 子テーブルの UUID カラムを本採用し、FK を張り直す
SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_uuid') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'form_choices'
       AND COLUMN_NAME = 'question_id') = 0,
    'ALTER TABLE form_choices CHANGE COLUMN question_uuid question_id CHAR(36) NOT NULL AFTER id',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

ALTER TABLE form_choices
    ADD CONSTRAINT fk_form_choices_question_id
        FOREIGN KEY IF NOT EXISTS (question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE;

SET @stmt := IF(
    (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_uuid') = 1
    AND (SELECT COUNT(*) FROM information_schema.COLUMNS
     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'real_answers'
       AND COLUMN_NAME = 'question_id') = 0,
    'ALTER TABLE real_answers CHANGE COLUMN question_uuid question_id CHAR(36) NOT NULL AFTER answer_id',
    'DO 0');
PREPARE guarded FROM @stmt; EXECUTE guarded; DEALLOCATE PREPARE guarded;

ALTER TABLE real_answers
    ADD CONSTRAINT fk_real_answers_question_id
        FOREIGN KEY IF NOT EXISTS (question_id) REFERENCES form_questions(question_id) ON DELETE CASCADE;

-- レガシーの fk_real_answers_answer_id には ON DELETE CASCADE がないため張り直して統一する。
-- レガシーで明示宣言されていたインデックスも落とし、FK の自動生成に任せて収束させる。
ALTER TABLE real_answers DROP FOREIGN KEY IF EXISTS fk_real_answers_answer_id;
ALTER TABLE real_answers DROP KEY IF EXISTS fk_real_answers_answer_id;
ALTER TABLE real_answers
    ADD CONSTRAINT fk_real_answers_answer_id
        FOREIGN KEY (answer_id) REFERENCES answers(id) ON DELETE CASCADE;

-- ============================================================================
-- Phase D: form_questions / form_choices の残りの是正
-- ============================================================================

-- question_type: レガシーの ENUM('TEXT','SINGLE','MULTIPLE') を VARCHAR(32) へ。
-- 既存値はドメイン層 (QuestionType) がそのままパースできる。NULL は TEXT に倒す。
UPDATE form_questions SET question_type = 'TEXT' WHERE question_type IS NULL;
ALTER TABLE form_questions MODIFY COLUMN question_type VARCHAR(32) NOT NULL;

-- template_key: UUID 由来の一意なキーでバックフィルする
-- (TemplateKey ドメイン型の制約 [A-Za-z0-9_-]{1,255} を満たす)
ALTER TABLE form_questions ADD COLUMN IF NOT EXISTS template_key VARCHAR(255) AFTER form_id;
UPDATE form_questions SET template_key = CONCAT('q-', question_id) WHERE template_key IS NULL;
ALTER TABLE form_questions MODIFY COLUMN template_key VARCHAR(255) NOT NULL AFTER form_id;
ALTER TABLE form_questions MODIFY COLUMN position SMALLINT UNSIGNED NOT NULL AFTER template_key;
ALTER TABLE form_questions
    ADD UNIQUE KEY IF NOT EXISTS uk_form_questions_form_id_template_key (form_id, template_key),
    ADD UNIQUE KEY IF NOT EXISTS uk_form_questions_form_id_position (form_id, position);

-- レガシーで明示宣言されていた form_id 単独のインデックスは、上の UNIQUE
-- (form_id が先頭カラム) が FK を担えるため冗長になる。落として収束させる。
ALTER TABLE form_questions DROP KEY IF EXISTS fk_form_questions_form_id;

-- form_choices: choice → label リネームと position の確定
ALTER TABLE form_choices CHANGE COLUMN IF EXISTS choice label TEXT NOT NULL;
ALTER TABLE form_choices MODIFY COLUMN position SMALLINT UNSIGNED NOT NULL AFTER question_id;
ALTER TABLE form_choices
    ADD UNIQUE KEY IF NOT EXISTS uk_form_choices_question_id_position (question_id, position);

-- ============================================================================
-- Phase E: レガシーテーブルの削除
-- (response_period / default_answer_titles は Phase A でバックフィル済み)
-- ============================================================================

DROP TABLE IF EXISTS response_period;
DROP TABLE IF EXISTS default_answer_titles;
DROP TABLE IF EXISTS form_webhooks;
DROP TABLE IF EXISTS seaql_migrations;
