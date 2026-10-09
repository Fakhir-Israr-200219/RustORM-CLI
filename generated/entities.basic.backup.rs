use rustorm::{
    entity::{Column, Entity, RelationKey},
    field::Field,
};

#[derive(Debug, sqlx::FromRow)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
}

pub struct User;

pub struct UserCreate {
    pub name: String,
}

impl rustorm::executor::InsertData<User> for UserCreate {
    fn columns(&self) -> &'static [&'static str] {
        &["name"]
    }

    fn values(&self) -> Vec<rustorm::value::BindValue> {
        vec![
            rustorm::value::BindValue::String(self.name.clone()),
        ]
    }
}

pub struct UserUpdate {
    pub name: Option<String>,
}

impl rustorm::executor::UpdateData<User> for UserUpdate {
    fn columns(&self) -> Vec<&'static str> {
        let mut columns = Vec::new();
        if self.name.is_some() {
            columns.push("name");
        }
        columns
    }

    fn values(&self) -> Vec<rustorm::value::BindValue> {
        let mut values = Vec::new();
        if let Some(value) = &self.name {
            values.push(rustorm::value::BindValue::String(value.clone()));
        }
        values
    }
}

impl Entity for User {
    type Model = UserModel;
    const TABLE: &'static str = "users";
    const COLUMNS: &'static [Column] = &[
        Column::new("id"),
        Column::new("name"),
    ];
}

impl User {
    #[allow(non_upper_case_globals)]
    pub const id: Field<Self, i32> = Field::new("id");

    #[allow(non_upper_case_globals)]
    pub const name: Field<Self, String> = Field::new("name");

}

impl RelationKey for UserModel {
    fn relation_key(&self, column: Column) -> Option<i64> {
        match column.name() {
            "id" => Some(self.id as i64),
            _ => None,
        }
    }
}

