CREATE USER IF NOT EXISTS 'user'@'%' IDENTIFIED BY 'password';
CREATE USER IF NOT EXISTS 'user'@'localhost' IDENTIFIED BY 'password';

GRANT ALL ON seichi_portal.* TO 'user'@'%';
GRANT ALL ON seichi_portal.* TO 'user'@'localhost';

CREATE USER 'debezium'@'%' IDENTIFIED BY 'debezium-password';
CREATE USER 'debezium'@'localhost' IDENTIFIED BY 'debezium-password';

GRANT ALL ON debezium.* TO 'debezium'@'%';
GRANT ALL ON debezium.* TO 'debezium'@'localhost';

GRANT SELECT, REPLICATION CLIENT, REPLICATION SLAVE, RELOAD ON *.* TO 'debezium'@'%';
GRANT SELECT, REPLICATION CLIENT, REPLICATION SLAVE, RELOAD ON *.* TO 'debezium'@'localhost';
