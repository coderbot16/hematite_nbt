#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Kind {
	End,
	I8,
	I16,
	I32,
	I64,
	F32,
	F64,
	I8Array,
	String,
	List,
	Compound,
	I32Array,
	I64Array
}

impl Kind {
	pub fn from_id(id: i8) -> Option<Self> {
		Some(match id {
			0 => Kind::End,
			1 => Kind::I8,
			2 => Kind::I16,
			3 => Kind::I32,
			4 => Kind::I64,
			5 => Kind::F32,
			6 => Kind::F64,
			7 => Kind::I8Array,
			8 => Kind::String,
			9 => Kind::List,
			10 => Kind::Compound,
			11 => Kind::I32Array,
			12 => Kind::I64Array,
			_ => return None
		})
	}
	
	pub fn to_id(&self) -> i8 {
		*self as i8
	}
	
	pub fn list_container(&self) -> Self {
		match *self {
			Kind::I8 => Kind::I8Array,
			Kind::I32 => Kind::I32Array,
			Kind::I64 => Kind::I64Array,
			_ => Kind::List
		}
	}
	
	pub fn is_list(&self) -> bool {
		match *self {
			Kind::List => true,
			Kind::I8Array => true,
			Kind::I32Array => true,
			Kind::I64Array => true,
			_ => false
		}
	}
}