INSERT INTO users (id, name, role)
VALUES
    ('478911be-3356-46c1-936e-fb14b71bf282', 'test_user', 'ADMINISTRATOR'),
    ('5cb955fb-5a05-4729-93ea-edcec7001001', 'test_standard_user', 'STANDARD_USER')
ON DUPLICATE KEY UPDATE
    name = VALUES(name),
    role = VALUES(role);
