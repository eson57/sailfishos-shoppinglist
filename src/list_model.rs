use chrono::{DateTime, Datelike as _, Local, NaiveDate, TimeZone, Timelike as _};
use cxx_qt_lib::{QDate, QDateTime, QString, QTime, QVariant, TimeSpec};
use uuid::Uuid;

pub trait ToVariant {
    fn to_variant(&self) -> QVariant;
}

macro_rules! direct_variants {
    ($($ty:ty),+ $(,)?) => {
        $(impl ToVariant for $ty {
            fn to_variant(&self) -> QVariant {
                QVariant::from(self)
            }
        })+
    };
}

direct_variants!(
    bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, QString, QDate, QTime, QDateTime,
);

impl ToVariant for str {
    fn to_variant(&self) -> QVariant {
        QVariant::from(&QString::from(self))
    }
}

impl ToVariant for String {
    fn to_variant(&self) -> QVariant {
        self.as_str().to_variant()
    }
}

impl ToVariant for Uuid {
    fn to_variant(&self) -> QVariant {
        self.to_string().to_variant()
    }
}

impl ToVariant for NaiveDate {
    fn to_variant(&self) -> QVariant {
        QVariant::from(&QDate::new(
            self.year(),
            self.month() as i32,
            self.day() as i32,
        ))
    }
}

impl<Tz: TimeZone> ToVariant for DateTime<Tz> {
    fn to_variant(&self) -> QVariant {
        let local = self.with_timezone(&Local);
        let date = QDate::new(local.year(), local.month() as i32, local.day() as i32);
        let time = QTime::new(
            local.hour() as i32,
            local.minute() as i32,
            local.second() as i32,
            0,
        );
        QVariant::from(&QDateTime::from_date_and_time_time_spec(
            &date,
            &time,
            TimeSpec::LocalTime,
            0,
        ))
    }
}

impl<T: ToVariant> ToVariant for Option<T> {
    fn to_variant(&self) -> QVariant {
        self.as_ref().map_or_else(QVariant::default, T::to_variant)
    }
}

/// Macro which generates a list model for an item type.
macro_rules! list_model {
    (
        model: $model:ident,
        rust: $rust:ident,
        roles: $roles:ident,
        row: $row:ty,
        new: $new:path,
        owned: $owned:ident,
        columns: [ $( ($variant:ident, $name:literal, $field:ident) ),+ $(,)? ] $(,)?
    ) => {
        #[derive(Default)]
        pub struct $rust {
            rows: Vec<$row>,
            count: i32,
        }

        impl $model {
            fn data(&self, index: &::cxx_qt_lib::QModelIndex, role: i32) -> ::cxx_qt_lib::QVariant {
                let Some(row) = usize::try_from(index.row())
                    .ok()
                    .and_then(|row| self.rows.get(row))
                else {
                    return ::cxx_qt_lib::QVariant::default();
                };

                match ($roles { repr: role }) {
                    $( $roles::$variant => {
                        $crate::list_model::ToVariant::to_variant(&row.$field)
                    } )+
                    _ => ::cxx_qt_lib::QVariant::default(),
                }
            }

            fn role_names(&self) -> ::cxx_qt_lib::QHash<::cxx_qt_lib::QHashPair_i32_QByteArray> {
                // QML caches these on first use, so they must never change.
                let mut names = ::cxx_qt_lib::QHash::default();
                $( names.insert($roles::$variant.repr, ::cxx_qt_lib::QByteArray::from($name)); )+
                names
            }

            fn row_count(&self, parent: &::cxx_qt_lib::QModelIndex) -> i32 {
                if parent.is_valid() { 0 } else { self.count }
            }

            fn qt_remove_rows(
                self: ::std::pin::Pin<&mut Self>,
                row: i32,
                count: i32,
                parent: &::cxx_qt_lib::QModelIndex,
            ) -> bool {
                let (Ok(row), Ok(count)) = (usize::try_from(row), usize::try_from(count)) else {
                    return false;
                };
                !parent.is_valid() && self.remove_rows(row, count)
            }

            fn qt_move_rows(
                self: ::std::pin::Pin<&mut Self>,
                source_parent: &::cxx_qt_lib::QModelIndex,
                source_row: i32,
                count: i32,
                destination_parent: &::cxx_qt_lib::QModelIndex,
                destination_child: i32,
            ) -> bool {
                let (Ok(source_row), Ok(count), Ok(destination_child)) = (
                    usize::try_from(source_row),
                    usize::try_from(count),
                    usize::try_from(destination_child),
                ) else {
                    return false;
                };
                // Currently only supporting internal moves because reasons
                !source_parent.is_valid()
                    && !destination_parent.is_valid()
                    && self.move_rows(source_row, count, destination_child)
            }

            #[allow(dead_code)]
            fn reset_rows(mut self: ::std::pin::Pin<&mut Self>, rows: Vec<$row>) {
                let count = rows.len() as i32;
                let count_changed = count != self.count;

                self.as_mut().reset_rows_inner(rows);

                if count_changed {
                    self.count_changed();
                }
            }

            #[allow(dead_code)]
            fn reset_rows_inner(mut self: ::std::pin::Pin<&mut Self>, rows: Vec<$row>) {
                use ::cxx_qt::CxxQtType as _;

                let count = rows.len() as i32;

                self.as_mut().begin_reset_model();
                {
                    let mut this = self.as_mut().rust_mut();
                    this.rows = rows;
                    this.count = count;
                }
                self.as_mut().end_reset_model();
            }

            #[allow(dead_code)]
            fn prepend_rows(mut self: ::std::pin::Pin<&mut Self>, rows: Vec<$row>) {
                use ::cxx_qt::CxxQtType as _;

                if rows.is_empty() {
                    return;
                }
                let added = rows.len() as i32;

                self.as_mut().begin_insert_rows(
                    &::cxx_qt_lib::QModelIndex::default(),
                    0,
                    added - 1,
                );
                {
                    let mut this = self.as_mut().rust_mut();
                    this.rows.splice(0..0, rows.into_iter().rev());
                    this.count += added;
                }
                self.as_mut().end_insert_rows();
                self.count_changed();
            }

            #[allow(dead_code)]
            fn insert_rows(mut self: ::std::pin::Pin<&mut Self>, row: usize, rows: Vec<$row>) -> bool {
                let inserted = self.as_mut().insert_rows_inner(row, rows);
                if inserted {
                    self.count_changed();
                }
                inserted
            }

            #[allow(dead_code)]
            fn insert_rows_inner(mut self: ::std::pin::Pin<&mut Self>, row: usize, rows: Vec<$row>) -> bool {
                use ::cxx_qt::CxxQtType as _;

                if rows.is_empty() || row > self.rows.len() {
                    return false;
                }
                let added = rows.len();

                self.as_mut().begin_insert_rows(
                    &::cxx_qt_lib::QModelIndex::default(),
                    row as i32,
                    (row + added - 1) as i32,
                );
                {
                    let mut this = self.as_mut().rust_mut();
                    this.rows.splice(row..row, rows);
                    this.count += added as i32;
                }
                self.as_mut().end_insert_rows();
                true
            }

            #[allow(dead_code)]
            fn remove_rows(mut self: ::std::pin::Pin<&mut Self>, row: usize, count: usize) -> bool {
                let removed = self.as_mut().remove_rows_inner(row, count);
                if removed {
                    self.count_changed();
                }
                removed
            }

            #[allow(dead_code)]
            fn remove_rows_inner(mut self: ::std::pin::Pin<&mut Self>, row: usize, count: usize) -> bool {
                use ::cxx_qt::CxxQtType as _;

                if count == 0 || row.checked_add(count).is_none_or(|end| end > self.rows.len()) {
                    return false;
                }

                self.as_mut().begin_remove_rows(
                    &::cxx_qt_lib::QModelIndex::default(),
                    row as i32,
                    (row + count - 1) as i32,
                );
                {
                    let mut this = self.as_mut().rust_mut();
                    this.rows.drain(row..row + count);
                    this.count -= count as i32;
                }
                self.as_mut().end_remove_rows();
                true
            }

            #[allow(dead_code)]
            fn move_rows(
                mut self: ::std::pin::Pin<&mut Self>,
                source_row: usize,
                count: usize,
                destination_child: usize,
            ) -> bool {
                use ::cxx_qt::CxxQtType as _;

                let len = self.rows.len();
                if count == 0
                    || source_row.checked_add(count).is_none_or(|end| end > len)
                    || destination_child > len
                    || (source_row..=source_row + count).contains(&destination_child)
                {
                    return false;
                }

                let root = ::cxx_qt_lib::QModelIndex::default();
                if !self.as_mut().begin_move_rows(
                    &root,
                    source_row as i32,
                    (source_row + count - 1) as i32,
                    &root,
                    destination_child as i32,
                ) {
                    return false;
                }
                {
                    let mut this = self.as_mut().rust_mut();
                    if destination_child > source_row {
                        this.rows[source_row..destination_child].rotate_left(count);
                    } else {
                        this.rows[destination_child..source_row + count].rotate_right(count);
                    }
                }
                self.as_mut().end_move_rows();
                true
            }

            #[allow(dead_code)]
            fn set_rows(mut self: ::std::pin::Pin<&mut Self>, row: usize, rows: Vec<$row>) -> bool {
                use ::cxx_qt::CxxQtType as _;

                let count = rows.len();
                if count == 0 || row.checked_add(count).is_none_or(|end| end > self.rows.len()) {
                    return false;
                }

                {
                    let mut this = self.as_mut().rust_mut();
                    for (slot, new) in this.rows[row..row + count].iter_mut().zip(rows) {
                        *slot = new;
                    }
                }

                let root = ::cxx_qt_lib::QModelIndex::default();
                let top_left = self.index(row as i32, 0, &root);
                let bottom_right = self.index((row + count - 1) as i32, 0, &root);

                self.as_mut().data_changed(
                    &top_left,
                    &bottom_right,
                    &::cxx_qt_lib::QVector::<i32>::default(),
                );
                true
            }

            #[allow(dead_code)]
            fn apply_patch(
                mut self: ::std::pin::Pin<&mut Self>,
                ops: Vec<::list_patch::Op<$row>>,
            ) -> bool {
                let count_before = self.count;
                let mut applied = true;

                for op in ops {
                    let ok = match op {
                        ::list_patch::Op::Reset(rows) => {
                            self.as_mut().reset_rows_inner(rows);
                            true
                        }
                        ::list_patch::Op::Remove { row, count } => {
                            self.as_mut().remove_rows_inner(row, count)
                        }
                        ::list_patch::Op::Move { row, count, destination } => {
                            self.as_mut().move_rows(row, count, destination)
                        }
                        ::list_patch::Op::Insert { row, rows } => {
                            self.as_mut().insert_rows_inner(row, rows)
                        }
                        ::list_patch::Op::Update { row, rows } => {
                            self.as_mut().set_rows(row, rows)
                        }
                    };
                    if !ok {
                        ::tracing::warn!("{} rejected a list patch op", stringify!($model));
                        applied = false;
                        break;
                    }
                }

                if self.count != count_before {
                    self.count_changed();
                }
                applied
            }

            #[allow(dead_code)]
            fn patch_rows<K: ::std::hash::Hash + Eq>(
                mut self: ::std::pin::Pin<&mut Self>,
                rows: Vec<$row>,
                key: impl Fn(&$row) -> K,
            ) {
                let ops = ::list_patch::plan(&self.rows, rows.clone(), key);
                if !self.as_mut().apply_patch(ops) {
                    self.reset_rows(rows);
                }
            }
        }

        struct $owned(::cxx::UniquePtr<$model>);

        impl Default for $owned {
            fn default() -> Self {
                Self($new())
            }
        }

        impl $owned {
            fn as_ptr(&self) -> *mut $model {
                self.0
                    .as_ref()
                    .map_or(::std::ptr::null_mut(), |model| model as *const $model as *mut $model)
            }

            #[allow(dead_code)]
            fn reset_rows(&mut self, rows: Vec<$row>) {
                self.0.pin_mut().reset_rows(rows);
            }

            #[allow(dead_code)]
            fn prepend_rows(&mut self, rows: Vec<$row>) {
                self.0.pin_mut().prepend_rows(rows);
            }

            #[allow(dead_code)]
            fn insert_rows(&mut self, row: usize, rows: Vec<$row>) -> bool {
                self.0.pin_mut().insert_rows(row, rows)
            }

            #[allow(dead_code)]
            fn remove_rows(&mut self, row: usize, count: usize) -> bool {
                self.0.pin_mut().remove_rows(row, count)
            }

            #[allow(dead_code)]
            fn move_rows(&mut self, source_row: usize, count: usize, destination_child: usize) -> bool {
                self.0.pin_mut().move_rows(source_row, count, destination_child)
            }

            #[allow(dead_code)]
            fn set_rows(&mut self, row: usize, rows: Vec<$row>) -> bool {
                self.0.pin_mut().set_rows(row, rows)
            }

            #[allow(dead_code)]
            fn patch_rows<K: ::std::hash::Hash + Eq>(&mut self, rows: Vec<$row>, key: impl Fn(&$row) -> K) {
                self.0.pin_mut().patch_rows(rows, key);
            }
        }
    };
}
