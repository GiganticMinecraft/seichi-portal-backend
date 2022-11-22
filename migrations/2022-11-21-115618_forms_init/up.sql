CREATE DATABASE IF NOT EXISTS forms;

USE forms;

CREATE TABLE IF NOT EXISTS forms(
    id INT AUTO_INCREMENT,
    name VARCHAR(80),
    PRIMARY KEY(id)
);

USE seichi_portal; -- dieselの関係上最後は.envで指定しているdbに選択を戻さないといけない
