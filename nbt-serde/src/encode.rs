use std::io;

use serde;
use serde::ser;

use byteorder::{BigEndian, WriteBytesExt};

use error::{Error, Result};
use kind::Kind;

enum LevelState {
	/// Writing a Compound at this level.
	InNamed { name: Option<String> },
	/// Writing a List/Array at this level.
	InList  { kind: Kind },
	/// A list is about to be written at this level.
	/// Whether name is None or Some specifies whether it is in a Named or List.
	List    { name: Option<String>, len: i32 }
}

impl LevelState {
	fn open_list(self, len: i32) -> (Self, Result<Option<LevelState>>) {
		match self {
			LevelState::InNamed { name } => {
				(LevelState::List { name: Some(name.expect("Key name not specified before value")), len }, Ok(None))
			},
			LevelState::InList { kind } => {
				if kind.is_list() {
					(LevelState::List { name: None, len }, Ok(None))
				} else {
					(self, Err(Error::HeterogenousList { original: kind, new: Kind::List }))
				}
			},
			LevelState::List { .. } => {
				(self, Ok(Some(LevelState::List { name: None, len })))
			}
		}
	}
	
	fn is_list(&self) -> bool {
		match self {
			&LevelState::List { .. } => true,
			_ => false
		}
	}
}

// TODO: Replace with a Trait on Write.
#[inline]
fn write_bare_string<W>(dst: &mut W, value: &str) -> Result<()> where W: io::Write
{    
    dst.write_u16::<BigEndian>(value.len() as u16)?;
    dst.write_all(value.as_bytes()).map_err(From::from)
}

/// Encode `value` in Named Binary Tag format to the given `io::Write`
/// destination, with an optional header.
#[inline]
pub fn to_writer<W, T>(dst: &mut W, value: &T, header: Option<String>)
                           -> Result<()>
    where W: ?Sized + io::Write,
          T: ?Sized + ser::Serialize,
{
    let mut encoder = Encoder::new(dst, header);
    value.serialize(&mut encoder)
}

/// Encode objects to Named Binary Tag format.
///
/// This structure can be used to serialize objects which implement the
/// `serde::Serialize` trait into NBT format. Note that not all types are
/// representable in NBT format (notably unsigned integers), so this encoder may
/// return errors.
pub struct Encoder<W> {
    writer: W,
    states: Vec<LevelState>,
}

impl<W> Encoder<W> where W: io::Write {

    /// Create an encoder with optional `header` from a given Writer.
    pub fn new(writer: W, header: Option<String>) -> Self {
    	let mut states = Vec::with_capacity(32);
    	states.push(LevelState::InNamed { name: Some(header.unwrap_or_else(|| "".to_string())) });
    	
        Encoder { writer, states }
    }

    /// Consume this encoder and return the underlying writer.
    #[inline]
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Write the NBT tag and an optional header to the underlying writer.
    #[inline]
    fn write_header(&mut self, tag: i8, header: Option<&str>) -> Result<()> {
        self.writer.write_i8(tag)?;
        match header {
            None    => self.writer.write_i16::<BigEndian>(0).map_err(From::from),
            Some(h) => write_bare_string(&mut self.writer, h).map_err(From::from)
        }
    }
    
    /// Specifies a kind at this level.
    fn specify_kind(&mut self, tag: Kind) -> Result<()> {
    	if tag.is_list() {
    		panic!("Encoder::specify_kind called with List, use Encoder::open_list instead.");
    	}
    	
    	let replacement = match self.states.pop().unwrap() {
    		LevelState::InNamed { name: None }           => panic!("value specified without key name"),
    		LevelState::InNamed { name: Some(ref name) } => {
    			self.writer.write_i8(tag.to_id())?;
    			write_bare_string(&mut self.writer, name).map_err(Error::from)?;
    			
    			self.states.push(LevelState::InNamed { name: None });
    		},
    		LevelState::InList { kind } => {
    			if kind != tag {
    				return Err(Error::HeterogenousList { original: kind, new: tag } )
    			} else {
    				self.states.push(LevelState::InList { kind });
    			}
    		},
    		LevelState::List { ref name, len } => {
    			match *name {
    				Some(ref name) => {
    					let container = tag.list_container();
    					
    					self.writer.write_i8(container.to_id())?;
		    			write_bare_string(&mut self.writer, name).map_err(Error::from)?;
    					if container == Kind::List {
    						self.writer.write_i8(tag.to_id())?;
    					}
    					self.writer.write_i32::<BigEndian>(len);
    					
    					self.states.push(LevelState::InNamed { name: None });
    					self.states.push(LevelState::InList { kind: tag });
    				},
    				None => {
    					// name = None replaced with InList, have to propagate change up the stack, child is InList { tag }
    					unimplemented!()
    				}
    			}
    		}
    	};
    	
    	if tag == Kind::Compound {
    		self.states.push(LevelState::InNamed { name: None });
    	}
    	
    	Ok(())
    }
    
    fn open_list(&mut self, len: i32) -> Result<()> {
    	let (push1, push2) = self.states.pop().unwrap().open_list(len);
    	let push2 = push2?;
    	
    	self.states.push(push1);
    	if let Some(push2) = push2 {
    		self.states.push(push2);
    	}
    	
    	Ok(())
    }
    
    /// Specifies the name at this level, only for InNamed.
    fn specify_name(&mut self, name: String) -> Result<()> {
    	match self.states.last_mut().unwrap() {
    		&mut LevelState::InNamed { name: ref mut current_name } => {
    			if current_name.is_none() {
    				*current_name = Some(name);
    				Ok(())
    			} else {
    				panic!("key name specified twice")
    			}
    		},
    		_ => panic!("key name specified while in a list")
    	}
    }
    
    fn cancel_name(&mut self) -> Result<()> {
    	match self.states.last_mut().unwrap() {
    		&mut LevelState::InNamed { name: ref mut current_name } => *current_name = None,
    		_ => ()
    	};
    	
    	Ok(())
    }
    
    /// Closes this level.
    fn close_level(&mut self) -> Result<()> {
    	if self.states.last().unwrap().is_list() {
    		self.specify_kind(Kind::End)?;
    	}
    	
    	match self.states.pop().unwrap() {
    		LevelState::InNamed { name } => {
    			if name.is_some() {
    				panic!("key name specified without value");
    			}
    			
    			self.writer.write_u8(0).map_err(From::from)
    		},
    		LevelState::InList  { kind } => Ok(()), // TODO: Check Length?
	    	_ => unreachable!()
    	}
    }
}

/// "Inner" version of the NBT encoder, capable of serializing bare types.
struct InnerEncoder<'a, W: 'a> {
    outer: &'a mut Encoder<W>,
}

#[doc(hidden)]
pub struct Compound<'a, W: 'a> {
    outer: &'a mut Encoder<W>
}

impl<'a, W> ser::SerializeSeq for Compound<'a, W>
    where W: io::Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
        where T: serde::Serialize
    {
        value.serialize(&mut InnerEncoder { outer: self.outer })
    }

    fn end(self) -> Result<()> {
        self.outer.close_level()
    }
}

impl<'a, W> ser::SerializeStruct for Compound<'a, W>
    where W: io::Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T)
                                  -> Result<()>
        where T: serde::Serialize
    {
    	self.outer.specify_name(key.to_owned())?;
        value.serialize(&mut InnerEncoder { outer: self.outer })
    }

    fn end(self) -> Result<()> {
        self.outer.close_level()
    }
}

impl<'a, W> serde::Serializer for &'a mut Encoder<W> where W: io::Write {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = Compound<'a, W>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    return_expr_for_serialized_types!(
        Err(Error::NoRootCompound); bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
            char str bytes none some unit unit_variant newtype_variant
            seq seq_fixed_size tuple tuple_struct tuple_variant struct_variant
    );

    /// Serialize unit structs as empty `Tag_Compound` data.
    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.specify_kind(Kind::Compound)?;
        self.close_level()
    }

    /// Serialize newtype structs by their underlying type. Note that this will
    /// only be successful if the underyling type is a struct.
    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T)
                                           -> Result<()>
        where T: ser::Serialize
    {
        value.serialize(self)
    }

    /// Arbitrary maps cannot be serialized, so calling this method will always
    /// return an error.
    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::UnrepresentableType("map"))
    }

    /// Serialize structs as `Tag_Compound` data.
    #[inline]
    fn serialize_struct(self, _name: &'static str, _len: usize)
                        -> Result<Self::SerializeStruct>
    {
        self.specify_kind(Kind::Compound)?;
        Ok(Compound { outer: self })
    }
}

impl<'a, W> serde::Serializer for &'a mut InnerEncoder<'a, W> where W: io::Write {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a, W>;
    type SerializeTuple = ser::Impossible<(), Error>;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = Compound<'a, W>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<()> {
        self.serialize_i8(value as i8)
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> Result<()> {
        self.outer.specify_kind(Kind::I8)?;
        self.outer.writer.write_i8(value).map_err(From::from)
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> Result<()> {
        self.outer.specify_kind(Kind::I16)?;
        self.outer.writer.write_i16::<BigEndian>(value).map_err(From::from)
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> Result<()> {
        self.outer.specify_kind(Kind::I32)?;
        self.outer.writer.write_i32::<BigEndian>(value).map_err(From::from)
    }

    #[inline]
    fn serialize_i64(self, value: i64) -> Result<()> {
        self.outer.specify_kind(Kind::I64)?;
        self.outer.writer.write_i64::<BigEndian>(value).map_err(From::from)
    }

    #[inline]
    fn serialize_u8(self, _value: u8) -> Result<()> {
        Err(Error::UnrepresentableType("u8"))
    }

    #[inline]
    fn serialize_u16(self, _value: u16) -> Result<()> {
        Err(Error::UnrepresentableType("u16"))
    }

    #[inline]
    fn serialize_u32(self, _value: u32) -> Result<()> {
        Err(Error::UnrepresentableType("u32"))
    }

    #[inline]
    fn serialize_u64(self, _value: u64) -> Result<()> {
        Err(Error::UnrepresentableType("u64"))
    }

    #[inline]
    fn serialize_f32(self, value: f32) -> Result<()> {
        self.outer.specify_kind(Kind::F32)?;
        self.outer.writer.write_f32::<BigEndian>(value).map_err(From::from)
    }

    #[inline]
    fn serialize_f64(self, value: f64) -> Result<()> {
        self.outer.specify_kind(Kind::F64)?;
        self.outer.writer.write_f64::<BigEndian>(value).map_err(From::from)
    }

    #[inline]
    fn serialize_char(self, _value: char) -> Result<()> {
        Err(Error::UnrepresentableType("char"))
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.outer.specify_kind(Kind::String)?;
        write_bare_string(&mut self.outer.writer, value).map_err(From::from)
    }

    #[inline]
    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(Error::UnrepresentableType("u8"))
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
    	self.outer.cancel_name();
        Ok(())
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<()>
        where T: ser::Serialize
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<()> {
        Err(Error::UnrepresentableType("unit"))
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.outer.specify_kind(Kind::Compound)?;
        self.outer.close_level()
    }

    #[inline]
    fn serialize_unit_variant(self, _name: &'static str, _index: usize,
                              _variant: &'static str) -> Result<()>
    {
        Err(Error::UnrepresentableType("unit variant"))
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T)
                                           -> Result<()>
        where T: ser::Serialize
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(self, _name: &'static str,
                                            _index: usize,
                                            _variant: &'static str,
                                            _value: &T) -> Result<()>
        where T: ser::Serialize
    {
        Err(Error::UnrepresentableType("newtype variant"))
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        if let Some(l) = len {
        	self.outer.open_list(l as i32)?;
        	
            Ok(Compound { outer: self.outer })
        } else {
            Err(Error::UnrepresentableType("unsized list"))
        }
    }

    #[inline]
    fn serialize_seq_fixed_size(self, len: usize) -> Result<Self::SerializeSeq>
    {
        self.outer.open_list(len as i32)?;
        Ok(Compound { outer: self.outer })
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::UnrepresentableType("tuple"))
    }

    #[inline]
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize)
                              -> Result<Self::SerializeTupleStruct>
    {
        Err(Error::UnrepresentableType("tuple struct"))
    }

    #[inline]
    fn serialize_tuple_variant(self, _name: &'static str, _index: usize,
                               _variant: &'static str, _len: usize)
                               -> Result<Self::SerializeTupleVariant>
    {
        Err(Error::UnrepresentableType("tuple variant"))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::UnrepresentableType("map"))
    }

    #[inline]
    fn serialize_struct(self, _name: &'static str, _len: usize)
                        -> Result<Self::SerializeStruct>
    {
        self.outer.specify_kind(Kind::Compound)?;
        Ok(Compound { outer: self.outer })
    }

    #[inline]
    fn serialize_struct_variant(self, _name: &'static str, _index: usize,
                                _variant: &'static str, _len: usize)
                                -> Result<Self::SerializeStructVariant>
    {
        Err(Error::UnrepresentableType("struct variant"))
    }
}
